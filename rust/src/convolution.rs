//! Convolution numérique 2D (lot « Filtrage spatial », Naomi).
//!
//! g(x, y) = Σ_j Σ_i  h(i, j) × f(x + i, y + j)      i, j ∈ [−r, r]   (r = taille / 2)
//!
//! Le noyau h est carré, de taille impaire (3×3, 5×5…), donné ligne par ligne.
//! (Le noyau n'est pas retourné : pour les noyaux symétriques du cours, corrélation et
//! convolution donnent le même résultat.)
//!
//! Gestion des bords, quand le noyau dépasse de l'image :
//!   - zero      : les pixels hors de l'image valent 0 (bord assombri)
//!   - partielle : on n'utilise que les coefficients qui tombent dans l'image ; avec la
//!                 normalisation, on divise par la somme de ces seuls coefficients
//!   - miroir    : l'image est prolongée par symétrie (f(−1) = f(1), f(n) = f(n−2))
//!
//! Normalisation : on divise par la somme des coefficients (si elle n'est pas nulle),
//! pour que l'image garde la même luminance (ex. moyenneur 3×3 : coefficients 1, somme 9).
//!
//! Le résultat est arrondi puis ramené dans [0, 255]. Une image couleur est traitée canal par canal.

use crate::pnm::{self, Image};
use crate::{verifier_arguments, Resultat};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bords {
    Zero,
    Partielle,
    Miroir,
}

impl Bords {
    pub fn depuis_texte(texte: &str) -> Result<Bords, String> {
        match texte {
            "zero" => Ok(Bords::Zero),
            "partielle" => Ok(Bords::Partielle),
            "miroir" => Ok(Bords::Miroir),
            autre => Err(format!("gestion des bords inconnue : « {autre} » (zero, partielle ou miroir)")),
        }
    }
}

/// Noyau carré de taille impaire, coefficients rangés ligne par ligne.
#[derive(Debug, Clone, PartialEq)]
pub struct Noyau {
    pub taille: usize,
    pub coefficients: Vec<f64>,
}

impl Noyau {
    pub fn nouveau(taille: usize, coefficients: Vec<f64>) -> Result<Noyau, String> {
        if taille == 0 || taille % 2 == 0 {
            return Err(format!("le noyau doit avoir une taille impaire (reçu {taille})"));
        }
        if coefficients.len() != taille * taille {
            return Err(format!("{} coefficients pour un noyau {taille}x{taille}", coefficients.len()));
        }
        Ok(Noyau { taille, coefficients })
    }

    /// Lit un noyau écrit « 1,2,1,2,4,2,1,2,1 » : 9 valeurs -> 3×3, 25 -> 5×5…
    pub fn depuis_texte(texte: &str) -> Result<Noyau, String> {
        let coefficients = texte
            .split(',')
            .map(|v| v.trim().parse::<f64>().map_err(|_| format!("coefficient invalide : « {} »", v.trim())))
            .collect::<Result<Vec<f64>, String>>()?;
        let taille = (coefficients.len() as f64).sqrt().round() as usize;
        if taille * taille != coefficients.len() || taille % 2 == 0 {
            return Err(format!(
                "le noyau doit être carré de taille impaire (9, 25, 49… coefficients), {} reçu(s)",
                coefficients.len()
            ));
        }
        Noyau::nouveau(taille, coefficients)
    }

    fn coef(&self, i: usize, j: usize) -> f64 {
        self.coefficients[j * self.taille + i]
    }
}

/// Indice « miroir » : prolonge [0, n[ par symétrie autour des pixels du bord.
fn miroir(mut i: i64, n: i64) -> i64 {
    if n == 1 {
        return 0;
    }
    let periode = 2 * (n - 1);
    i = i.rem_euclid(periode);
    if i >= n {
        periode - i
    } else {
        i
    }
}

/// Convolution d'une image (gris ou couleur) par un noyau.
pub fn convoluer(image: &Image, noyau: &Noyau, bords: Bords, normaliser: bool) -> Image {
    let r = (noyau.taille / 2) as i64;
    let (l, h) = (image.largeur as i64, image.hauteur as i64);
    let somme_noyau: f64 = noyau.coefficients.iter().sum();
    let mut sortie = Image::nouvelle(image.largeur, image.hauteur, image.canaux);

    for y in 0..h {
        for x in 0..l {
            for c in 0..image.canaux {
                let mut somme = 0.0;
                let mut somme_coefs_utilises = 0.0;
                for j in -r..=r {
                    for i in -r..=r {
                        let k = noyau.coef((i + r) as usize, (j + r) as usize);
                        let (mut xx, mut yy) = (x + i, y + j);
                        let dedans = xx >= 0 && xx < l && yy >= 0 && yy < h;
                        if !dedans {
                            match bords {
                                Bords::Zero | Bords::Partielle => continue, // contribue 0
                                Bords::Miroir => {
                                    xx = miroir(xx, l);
                                    yy = miroir(yy, h);
                                }
                            }
                        }
                        somme += k * image.valeur(xx as usize, yy as usize, c) as f64;
                        somme_coefs_utilises += k;
                    }
                }
                if normaliser {
                    // En « partielle », seule la partie du noyau dans l'image compte
                    let diviseur = if bords == Bords::Partielle { somme_coefs_utilises } else { somme_noyau };
                    if diviseur.abs() > 1e-12 {
                        somme /= diviseur;
                    }
                }
                sortie.modifier(x as usize, y as usize, c, somme.round().clamp(0.0, 255.0) as u8);
            }
        }
    }
    sortie
}

/// `traitements convolution <entrée> <sortie> <noyau "a,b,c,…"> <zero|partielle|miroir> <normaliser 1|0>`
pub fn executer(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée", "sortie", "noyau", "bords (zero|partielle|miroir)", "normaliser (1|0)"])?;
    let image = pnm::lire(&args[0])?;
    let noyau = Noyau::depuis_texte(&args[2])?;
    let bords = Bords::depuis_texte(&args[3])?;
    let normaliser = args[4] == "1" || args[4] == "true";
    pnm::ecrire(&args[1], &convoluer(&image, &noyau, bords, normaliser))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gris(largeur: usize, hauteur: usize, pixels: Vec<u8>) -> Image {
        Image::depuis_pixels(largeur, hauteur, 1, pixels).unwrap()
    }

    #[test]
    fn noyau_identite_ne_change_rien() {
        let image = gris(3, 2, vec![1, 2, 3, 4, 5, 6]);
        let id = Noyau::depuis_texte("0,0,0,0,1,0,0,0,0").unwrap();
        for b in [Bords::Zero, Bords::Partielle, Bords::Miroir] {
            assert_eq!(convoluer(&image, &id, b, true), image);
        }
    }

    #[test]
    fn moyenne_3x3_calculee_a_la_main() {
        // Image 3×3, centre : (10+20+30+40+50+60+70+80+90) / 9 = 50
        let image = gris(3, 3, vec![10, 20, 30, 40, 50, 60, 70, 80, 90]);
        let moy = Noyau::depuis_texte("1,1,1,1,1,1,1,1,1").unwrap();
        let r = convoluer(&image, &moy, Bords::Partielle, true);
        assert_eq!(r.valeur(1, 1, 0), 50);
        // Coin haut-gauche en « partielle » : (10+20+40+50) / 4 = 30
        assert_eq!(r.valeur(0, 0, 0), 30);
        // Même coin en « zero » : (10+20+40+50) / 9 = 13,3 -> 13
        assert_eq!(convoluer(&image, &moy, Bords::Zero, true).valeur(0, 0, 0), 13);
        // Même coin en « miroir » : voisins 50,40,50 / 20,10,20 / 50,40,50 -> 330/9 = 36,7 -> 37
        assert_eq!(convoluer(&image, &moy, Bords::Miroir, true).valeur(0, 0, 0), 37);
    }

    #[test]
    fn sans_normalisation_resultat_sature() {
        let image = gris(3, 3, vec![100; 9]);
        let moy = Noyau::depuis_texte("1,1,1,1,1,1,1,1,1").unwrap();
        assert_eq!(convoluer(&image, &moy, Bords::Miroir, false).valeur(1, 1, 0), 255);
    }

    #[test]
    fn indices_miroir() {
        assert_eq!(miroir(-1, 5), 1);
        assert_eq!(miroir(-2, 5), 2);
        assert_eq!(miroir(5, 5), 3);
        assert_eq!(miroir(6, 5), 2);
        assert_eq!(miroir(3, 1), 0);
    }

    #[test]
    fn noyau_mal_forme_refuse() {
        assert!(Noyau::depuis_texte("1,2,3,4").is_err()); // 2×2 : pas impair
        assert!(Noyau::depuis_texte("1,2,3").is_err()); // pas carré
        assert!(Noyau::depuis_texte("1,a,1,1,1,1,1,1,1").is_err());
    }
}
