//! Filtrage spatial (lot « Filtrage spatial », Naomi).
//!
//! Filtres linéaires (convolution, voir `convolution.rs`) :
//!   - moyenneur N×N  : tous les coefficients à 1, normalisé par N²       (passe-bas, lissage)
//!   - gaussien       : h(i, j) = exp(−(i² + j²) / (2σ²)), normalisé      (passe-bas, lissage)
//!                      taille du noyau = 2 × ⌈3σ⌉ + 1 (99 % de la gaussienne)
//!   - passe-haut     : accentuation = image − laplacien ; noyaux à somme 1 :
//!                        4 voisins :  0 −1  0 / −1  5 −1 /  0 −1  0
//!                        8 voisins : −1 −1 −1 / −1  9 −1 / −1 −1 −1
//!
//! Filtres non linéaires (ce ne sont PAS des convolutions) sur une fenêtre N×N :
//!   - médian : la valeur du milieu des valeurs triées (efface le bruit poivre et sel)
//!   - min    : la plus petite valeur (érode les zones claires)
//!   - max    : la plus grande valeur (érode les zones sombres)
//!   Au bord, la fenêtre est coupée : seuls les pixels de l'image comptent.
//!
//! Bruit poivre et sel : une proportion p des pixels devient noire (0) ou blanche (255),
//! moitié-moitié en moyenne. Générateur pseudo-aléatoire simple (xorshift) à graine fixée,
//! pour obtenir toujours le même bruit avec la même graine.
//!
//! Une image couleur est traitée canal par canal (le bruit touche les trois canaux du pixel).

use crate::convolution::{convoluer, Bords, Noyau};
use crate::pnm::{self, Image};
use crate::{verifier_arguments, Resultat};

// ---------------------------------------------------------------------------
// Filtres linéaires
// ---------------------------------------------------------------------------

/// Noyau moyenneur N×N (coefficients à 1).
pub fn noyau_moyenneur(taille: usize) -> Result<Noyau, String> {
    Noyau::nouveau(taille, vec![1.0; taille * taille])
}

/// Noyau gaussien d'écart-type `sigma`.
pub fn noyau_gaussien(sigma: f64) -> Result<Noyau, String> {
    if !(sigma > 0.0) {
        return Err(format!("sigma doit être > 0 (reçu {sigma})"));
    }
    let r = (3.0 * sigma).ceil() as i64;
    let taille = (2 * r + 1) as usize;
    let mut coefficients = Vec::with_capacity(taille * taille);
    for j in -r..=r {
        for i in -r..=r {
            coefficients.push((-((i * i + j * j) as f64) / (2.0 * sigma * sigma)).exp());
        }
    }
    Noyau::nouveau(taille, coefficients)
}

/// Noyau passe-haut d'accentuation (4 ou 8 voisins).
pub fn noyau_passe_haut(voisins: u8) -> Result<Noyau, String> {
    match voisins {
        4 => Noyau::nouveau(3, vec![0.0, -1.0, 0.0, -1.0, 5.0, -1.0, 0.0, -1.0, 0.0]),
        8 => Noyau::nouveau(3, vec![-1.0, -1.0, -1.0, -1.0, 9.0, -1.0, -1.0, -1.0, -1.0]),
        _ => Err(format!("voisinage inconnu : {voisins} (4 ou 8)")),
    }
}

// ---------------------------------------------------------------------------
// Filtres non linéaires (filtres d'ordre)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ordre {
    Median,
    Min,
    Max,
}

/// Applique un filtre d'ordre (médian, min ou max) sur une fenêtre `taille` × `taille`.
pub fn filtre_ordre(image: &Image, taille: usize, ordre: Ordre) -> Image {
    let r = taille / 2;
    let mut sortie = Image::nouvelle(image.largeur, image.hauteur, image.canaux);
    let mut fenetre: Vec<u8> = Vec::with_capacity(taille * taille);

    for y in 0..image.hauteur {
        let (y0, y1) = (y.saturating_sub(r), (y + r).min(image.hauteur - 1));
        for x in 0..image.largeur {
            let (x0, x1) = (x.saturating_sub(r), (x + r).min(image.largeur - 1));
            for c in 0..image.canaux {
                fenetre.clear();
                for yy in y0..=y1 {
                    for xx in x0..=x1 {
                        fenetre.push(image.valeur(xx, yy, c));
                    }
                }
                let v = match ordre {
                    Ordre::Min => *fenetre.iter().min().expect("fenêtre non vide"),
                    Ordre::Max => *fenetre.iter().max().expect("fenêtre non vide"),
                    Ordre::Median => {
                        fenetre.sort_unstable();
                        fenetre[fenetre.len() / 2]
                    }
                };
                sortie.modifier(x, y, c, v);
            }
        }
    }
    sortie
}

// ---------------------------------------------------------------------------
// Bruit poivre et sel
// ---------------------------------------------------------------------------

/// Générateur pseudo-aléatoire xorshift64 (bibliothèque standard uniquement).
struct Alea(u64);

impl Alea {
    fn nouveau(graine: u64) -> Alea {
        // L'état ne doit jamais valoir 0 ; on mélange la graine (constante de SplitMix64)
        Alea(graine.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    fn suivant(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// Nombre réel dans [0, 1[
    fn reel(&mut self) -> f64 {
        (self.suivant() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Ajoute du bruit poivre et sel sur une proportion `proportion` (0..1) des pixels.
pub fn poivre_et_sel(image: &Image, proportion: f64, graine: u64) -> Image {
    let mut sortie = image.clone();
    let mut alea = Alea::nouveau(graine);
    for y in 0..image.hauteur {
        for x in 0..image.largeur {
            if alea.reel() < proportion {
                let v = if alea.reel() < 0.5 { 0 } else { 255 };
                for c in 0..image.canaux {
                    sortie.modifier(x, y, c, v);
                }
            }
        }
    }
    sortie
}

// ---------------------------------------------------------------------------
// Lecture des paramètres et sous-commandes
// ---------------------------------------------------------------------------

fn lire_taille(texte: &str) -> Result<usize, String> {
    let taille: usize = texte.parse().map_err(|_| format!("taille invalide : « {texte} » (entier attendu)"))?;
    if taille < 1 || taille % 2 == 0 || taille > 51 {
        return Err(format!("la taille doit être un entier impair entre 1 et 51 (reçu {taille})"));
    }
    Ok(taille)
}

/// `traitements moyenneur <entrée> <sortie> <taille> <zero|partielle|miroir>`
pub fn executer_moyenneur(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée", "sortie", "taille", "bords (zero|partielle|miroir)"])?;
    let image = pnm::lire(&args[0])?;
    let noyau = noyau_moyenneur(lire_taille(&args[2])?)?;
    let bords = Bords::depuis_texte(&args[3])?;
    pnm::ecrire(&args[1], &convoluer(&image, &noyau, bords, true))?;
    Ok(())
}

/// `traitements gaussien <entrée> <sortie> <sigma> <zero|partielle|miroir>`
pub fn executer_gaussien(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée", "sortie", "sigma", "bords (zero|partielle|miroir)"])?;
    let image = pnm::lire(&args[0])?;
    let sigma: f64 = args[2].parse().map_err(|_| format!("sigma invalide : « {} » (nombre attendu)", args[2]))?;
    if !(sigma > 0.0 && sigma <= 10.0) {
        return Err(format!("sigma doit être dans ]0, 10] (reçu {sigma})").into());
    }
    let noyau = noyau_gaussien(sigma)?;
    let bords = Bords::depuis_texte(&args[3])?;
    pnm::ecrire(&args[1], &convoluer(&image, &noyau, bords, true))?;
    Ok(())
}

/// `traitements passe-haut <entrée> <sortie> <4|8> <zero|partielle|miroir>`
pub fn executer_passe_haut(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée", "sortie", "voisinage (4|8)", "bords (zero|partielle|miroir)"])?;
    let image = pnm::lire(&args[0])?;
    let voisins: u8 = args[2].parse().map_err(|_| format!("voisinage invalide : « {} » (4 ou 8)", args[2]))?;
    let noyau = noyau_passe_haut(voisins)?;
    let bords = Bords::depuis_texte(&args[3])?;
    // Somme des coefficients = 1 : la normalisation ne change rien, on la garde pour la lisibilité
    pnm::ecrire(&args[1], &convoluer(&image, &noyau, bords, true))?;
    Ok(())
}

fn executer_ordre(args: &[String], ordre: Ordre) -> Resultat {
    verifier_arguments(args, &["entrée", "sortie", "taille"])?;
    let image = pnm::lire(&args[0])?;
    let taille = lire_taille(&args[2])?;
    pnm::ecrire(&args[1], &filtre_ordre(&image, taille, ordre))?;
    Ok(())
}

/// `traitements median <entrée> <sortie> <taille>`
pub fn executer_median(args: &[String]) -> Resultat {
    executer_ordre(args, Ordre::Median)
}

/// `traitements minimum <entrée> <sortie> <taille>`
pub fn executer_min(args: &[String]) -> Resultat {
    executer_ordre(args, Ordre::Min)
}

/// `traitements maximum <entrée> <sortie> <taille>`
pub fn executer_max(args: &[String]) -> Resultat {
    executer_ordre(args, Ordre::Max)
}

/// `traitements poivre-et-sel <entrée> <sortie> <proportion 0..1> <graine>`
pub fn executer_poivre_et_sel(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée", "sortie", "proportion", "graine"])?;
    let image = pnm::lire(&args[0])?;
    let proportion: f64 = args[2].parse().map_err(|_| format!("proportion invalide : « {} » (nombre entre 0 et 1)", args[2]))?;
    if !(0.0..=1.0).contains(&proportion) {
        return Err(format!("la proportion doit être entre 0 et 1 (reçu {proportion})").into());
    }
    let graine: u64 = args[3].parse().map_err(|_| format!("graine invalide : « {} » (entier ≥ 0 attendu)", args[3]))?;
    pnm::ecrire(&args[1], &poivre_et_sel(&image, proportion, graine))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gris(largeur: usize, hauteur: usize, pixels: Vec<u8>) -> Image {
        Image::depuis_pixels(largeur, hauteur, 1, pixels).unwrap()
    }

    #[test]
    fn median_efface_un_point_isole() {
        // Un pixel blanc (bruit « sel ») au milieu d'une zone à 50 : le médian 3×3 le supprime
        let mut px = vec![50u8; 9];
        px[4] = 255;
        let image = gris(3, 3, px);
        assert_eq!(filtre_ordre(&image, 3, Ordre::Median).valeur(1, 1, 0), 50);
    }

    #[test]
    fn min_et_max_calcules_a_la_main() {
        let image = gris(3, 3, vec![9, 1, 7, 3, 5, 8, 2, 6, 4]);
        assert_eq!(filtre_ordre(&image, 3, Ordre::Min).valeur(1, 1, 0), 1);
        assert_eq!(filtre_ordre(&image, 3, Ordre::Max).valeur(1, 1, 0), 9);
        assert_eq!(filtre_ordre(&image, 3, Ordre::Median).valeur(1, 1, 0), 5);
        // Coin (0,0) : fenêtre coupée {9, 1, 3, 5} -> min 1, max 9, médian (trié 1,3,5,9) -> 5
        assert_eq!(filtre_ordre(&image, 3, Ordre::Min).valeur(0, 0, 0), 1);
        assert_eq!(filtre_ordre(&image, 3, Ordre::Median).valeur(0, 0, 0), 5);
    }

    #[test]
    fn noyau_gaussien_symetrique_et_centre_maximal() {
        let n = noyau_gaussien(1.0).unwrap();
        assert_eq!(n.taille, 7); // 2 × ⌈3⌉ + 1
        let centre = n.coefficients[n.coefficients.len() / 2];
        assert_eq!(centre, 1.0);
        assert!(n.coefficients.iter().all(|&c| c <= centre));
        assert_eq!(n.coefficients[0], n.coefficients[n.coefficients.len() - 1]);
    }

    #[test]
    fn passe_haut_conserve_une_zone_uniforme_et_accentue_un_bord() {
        let image = gris(3, 3, vec![80; 9]);
        let n4 = noyau_passe_haut(4).unwrap();
        assert_eq!(convoluer(&image, &n4, Bords::Miroir, true).pixels, vec![80; 9]);
        // Point plus clair au centre : 5×100 − 4×80 = 180
        let mut px = vec![80u8; 9];
        px[4] = 100;
        let image = gris(3, 3, px);
        assert_eq!(convoluer(&image, &n4, Bords::Miroir, true).valeur(1, 1, 0), 180);
    }

    #[test]
    fn moyenneur_lisse_un_damier() {
        let image = gris(3, 3, vec![0, 255, 0, 255, 0, 255, 0, 255, 0]);
        let n = noyau_moyenneur(3).unwrap();
        // centre : (4 × 255) / 9 = 113,3 -> 113
        assert_eq!(convoluer(&image, &n, Bords::Partielle, true).valeur(1, 1, 0), 113);
    }

    #[test]
    fn poivre_et_sel_proportion_et_reproductibilite() {
        let image = gris(100, 100, vec![128; 10_000]);
        let a = poivre_et_sel(&image, 0.1, 42);
        let b = poivre_et_sel(&image, 0.1, 42);
        assert_eq!(a, b); // même graine -> même bruit
        let bruites = a.pixels.iter().filter(|&&v| v != 128).count();
        assert!((800..1200).contains(&bruites), "{bruites} pixels bruités");
        assert!(a.pixels.iter().all(|&v| v == 0 || v == 128 || v == 255));
        assert_eq!(poivre_et_sel(&image, 0.0, 1), image);
    }

    #[test]
    fn tailles_invalides_refusees() {
        assert!(lire_taille("4").is_err());
        assert!(lire_taille("0").is_err());
        assert!(lire_taille("x").is_err());
        assert_eq!(lire_taille("5"), Ok(5));
    }
}
