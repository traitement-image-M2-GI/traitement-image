//! Seuillage local adaptatif (chapitre « Segmentation »).
//!
//! Pour chaque pixel, le seuil est calculé sur la fenêtre NxN centrée en ce pixel :
//!   seuil = moyenne_locale + constante + poids_variance * ecart_type_local
//! Le paramètre `poids_variance` permet d'utiliser le critère de variance vu en cours.
//! Aux bords, la fenêtre est coupée : seuls les pixels dans l'image sont comptés.

use crate::gris;
use crate::pnm::{self, Image};
use crate::{verifier_arguments, Resultat};

/// Binarise l'image avec un seuil recalculé autour de chaque pixel.
pub fn appliquer(image: &Image, fenetre: usize, constante: f64, poids_variance: f64) -> Image {
    let gris = gris::vers_gris(image);
    let r = fenetre / 2;
    let mut sortie = Image::nouvelle(image.largeur, image.hauteur, 1);

    for y in 0..gris.hauteur {
        let (y0, y1) = (y.saturating_sub(r), (y + r).min(gris.hauteur - 1));
        for x in 0..gris.largeur {
            let (x0, x1) = (x.saturating_sub(r), (x + r).min(gris.largeur - 1));
            let mut somme = 0u64;
            let mut somme_cares = 0u64;
            let mut total = 0u64;
            for yy in y0..=y1 {
                for xx in x0..=x1 {
                    let v = gris.valeur(xx, yy, 0) as u64;
                    somme += v;
                    somme_cares += v * v;
                    total += 1;
                }
            }
            let moyenne = somme as f64 / total as f64;
            let variance = (somme_cares as f64 / total as f64 - moyenne * moyenne).max(0.0);
            let seuil = moyenne + constante + poids_variance * variance.sqrt();
            sortie.modifier(x, y, 0, if gris.valeur(x, y, 0) as f64 >= seuil { 255 } else { 0 });
        }
    }
    sortie
}

/// `traitements seuillage-local <entrée> <sortie.pnm> <fenêtre> <constante> <poids_variance>`
pub fn executer(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée", "sortie.pnm", "fenêtre", "constante", "poids_variance"])?;
    let image = pnm::lire(&args[0])?;
    let fenetre: usize = args[2].parse().map_err(|_| format!("fenêtre invalide : « {} »", args[2]))?;
    if fenetre < 3 || fenetre % 2 == 0 {
        return Err(format!("la fenêtre doit être un entier impair ≥ 3 (reçu {fenetre})").into());
    }
    let constante: f64 = args[3].parse().map_err(|_| format!("constante invalide : « {} »", args[3]))?;
    let poids: f64 = args[4].parse().map_err(|_| format!("poids de variance invalide : « {} »", args[4]))?;
    pnm::ecrire(&args[1], &appliquer(&image, fenetre, constante, poids))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exemple_fait_a_la_main() {
        // Fenêtre 3x3 sur une image 3x3 : la moyenne vaut 4 pour tous les pixels.
        let image = Image::depuis_pixels(3, 3, 1, (0..9).map(|v| v as u8).collect()).unwrap();
        assert_eq!(appliquer(&image, 3, 0.0, 0.0).pixels, [0, 0, 0, 0, 255, 255, 255, 255, 255]);
    }

    #[test]
    fn une_constante_plus_haute_tout_passe_au_fond() {
        let image = Image::depuis_pixels(3, 3, 1, (0..9).map(|v| v as u8).collect()).unwrap();
        assert!(appliquer(&image, 3, 10.0, 0.0).pixels.iter().all(|&v| v == 0));
    }

    #[test]
    fn bord_ne_plante_pas() {
        let image = Image::depuis_pixels(2, 2, 1, vec![0, 255, 255, 0]).unwrap();
        assert_eq!(appliquer(&image, 7, 0.0, 0.0).pixels.len(), 4);
    }
}
