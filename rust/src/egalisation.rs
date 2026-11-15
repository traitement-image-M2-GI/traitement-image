//! Égalisation d'histogramme (cours 02 « Traitement de base », diapos 20 à 24).
//!
//! But : aplanir l'histogramme pour que l'image utilise toute la plage 0..255.
//!
//! Égalisation globale, les 4 étapes de la diapo 21 :
//!   1. h(i)  = nombre de pixels de valeur i
//!   2. hn(i) = h(i) / Nbp                       (Nbp = nombre de pixels)
//!   3. C(i)  = hn(0) + hn(1) + … + hn(i)        (proportion des pixels ≤ i)
//!   4. f'(x,y) = C(f(x,y)) × 255
//!
//! Égalisation locale (diapo 24) : même principe, mais C est calculé sur la fenêtre N×N
//! centrée sur chaque pixel, et appliqué uniquement au pixel central :
//!   f'(x,y) = 255 × (nb de pixels de la fenêtre ≤ f(x,y)) / (nb de pixels de la fenêtre)

use crate::histogramme;
use crate::pnm::{self, Image};
use crate::{verifier_arguments, Resultat};

/// Égalisation globale d'une image en niveaux de gris.
pub fn globale(image: &Image) -> Image {
    // Étape 1 : histogramme
    let h = histogramme::calculer(image, 0);
    let nbp = image.nb_pixels() as u64;

    // Étapes 2 à 4 regroupées dans une table de correspondance (LUT, diapo 11) :
    // lut[i] = arrondi(C(i) × 255) = arrondi(cumul(i) × 255 / Nbp).
    // Calcul en entiers (u64 : pas de dépassement) ; « + nbp / 2 » sert à arrondir au lieu de tronquer.
    let mut lut = [0u8; 256];
    let mut cumul = 0u64;
    for i in 0..256 {
        cumul += h[i];
        lut[i] = ((cumul * 255 + nbp / 2) / nbp) as u8;
    }

    // Étape 4 : chaque pixel prend la valeur de la table
    let pixels = image.pixels.iter().map(|&v| lut[v as usize]).collect();
    Image::depuis_pixels(image.largeur, image.hauteur, 1, pixels).expect("même taille que l'entrée")
}

/// Égalisation locale : fenêtre `fenetre` × `fenetre` (impaire) autour de chaque pixel.
/// Au bord, la fenêtre est coupée : on ne compte que les pixels qui sont dans l'image.
pub fn locale(image: &Image, fenetre: usize) -> Image {
    let r = fenetre / 2; // rayon : 7 -> 3 pixels de chaque côté
    let mut sortie = Image::nouvelle(image.largeur, image.hauteur, 1);

    for y in 0..image.hauteur {
        // lignes de la fenêtre, coupées aux bords. saturating_sub : 0 - 3 donne 0 au lieu de planter
        let (y0, y1) = (y.saturating_sub(r), (y + r).min(image.hauteur - 1));
        for x in 0..image.largeur {
            let (x0, x1) = (x.saturating_sub(r), (x + r).min(image.largeur - 1));
            let v = image.valeur(x, y, 0); // on lit toujours l'image d'ORIGINE

            let mut inferieurs = 0u64; // pixels de la fenêtre ≤ v
            for yy in y0..=y1 {
                for xx in x0..=x1 {
                    if image.valeur(xx, yy, 0) <= v {
                        inferieurs += 1;
                    }
                }
            }
            let total = ((y1 - y0 + 1) * (x1 - x0 + 1)) as u64;
            sortie.modifier(x, y, 0, ((inferieurs * 255 + total / 2) / total) as u8);
        }
    }
    sortie
}

/// `traitements egalisation <entrée.pgm> <sortie.pgm> <globale|locale> <fenêtre>`
pub fn executer(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée.pgm", "sortie.pgm", "mode (globale|locale)", "fenêtre"])?;
    let image = pnm::lire(&args[0])?;
    if image.est_couleur() {
        return Err("une image en niveaux de gris (PGM) est attendue".into());
    }
    let fenetre: usize = args[3].parse().map_err(|_| format!("fenêtre invalide : « {} » (entier attendu)", args[3]))?;

    let resultat = match args[2].as_str() {
        "globale" => globale(&image),
        "locale" => {
            if fenetre < 3 || fenetre % 2 == 0 {
                return Err(format!("la fenêtre doit être un entier impair ≥ 3 (reçu {fenetre})").into());
            }
            locale(&image, fenetre)
        }
        autre => return Err(format!("mode inconnu : « {autre} » (globale ou locale)").into()),
    };
    pnm::ecrire(&args[1], &resultat)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gris(largeur: usize, hauteur: usize, pixels: Vec<u8>) -> Image {
        Image::depuis_pixels(largeur, hauteur, 1, pixels).unwrap()
    }

    #[test]
    fn exemple_fait_a_la_main() {
        // L'exemple expliqué : 100 -> 96, 101 -> 159, 102 -> 191, 103 -> 255
        let image = gris(8, 1, vec![100, 100, 100, 101, 101, 102, 103, 103]);
        assert_eq!(globale(&image).pixels, vec![96, 96, 96, 159, 159, 191, 255, 255]);
    }

    #[test]
    fn le_resultat_ne_depend_que_de_l_ordre_des_pixels() {
        // Diapo 23 : même image, contrastes différents -> même résultat
        let terne = gris(4, 1, vec![120, 121, 122, 123]);
        let contrastee = gris(4, 1, vec![0, 80, 160, 240]);
        assert_eq!(globale(&terne), globale(&contrastee));
    }

    #[test]
    fn la_valeur_la_plus_claire_devient_255() {
        let image = gris(3, 1, vec![5, 50, 60]);
        assert_eq!(*globale(&image).pixels.iter().max().unwrap(), 255);
    }

    #[test]
    fn locale_avec_grande_fenetre_egale_globale() {
        // Fenêtre plus grande que l'image : elle couvre toute l'image pour chaque pixel
        let image = gris(3, 3, vec![10, 20, 30, 40, 50, 60, 70, 80, 90]);
        assert_eq!(locale(&image, 7), globale(&image));
    }

    #[test]
    fn locale_au_bord_ne_plante_pas() {
        let image = gris(2, 2, vec![0, 255, 255, 0]);
        let resultat = locale(&image, 3);
        assert_eq!(resultat.pixels.len(), 4);
    }
}
