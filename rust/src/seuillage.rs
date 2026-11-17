//! Seuillage simple (chapitre « Segmentation »).
//!
//! f'(x,y) = 255 si f(x,y) >= S, 0 sinon.
//! Une image couleur est d'abord convertie en niveaux de gris (luminance BT.601) ;
//! la sortie est toujours un PGM binaire (1 canal, valeurs 0 ou 255).

use crate::gris;
use crate::pnm::{self, Image};
use crate::{verifier_arguments, Resultat};

/// Binarise une image : les pixels >= seuil deviennent 255 (objet), les autres 0 (fond).
pub fn appliquer(image: &Image, seuil: u8) -> Image {
    let gris = gris::vers_gris(image);
    let pixels = gris.pixels.iter().map(|&v| if v >= seuil { 255 } else { 0 }).collect();
    Image::depuis_pixels(image.largeur, image.hauteur, 1, pixels).expect("même nombre de pixels que l'entrée")
}

/// `traitements seuillage <entrée> <sortie.pnm> <seuil>`
pub fn executer(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée", "sortie.pnm", "seuil"])?;
    let image = pnm::lire(&args[0])?;
    let seuil: u8 = args[2]
        .parse()
        .map_err(|_| format!("seuil invalide : « {} » (entier entre 0 et 255 attendu)", args[2]))?;
    pnm::ecrire(&args[1], &appliquer(&image, seuil))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exemple_fait_a_la_main() {
        let image = Image::depuis_pixels(5, 1, 1, vec![0, 50, 127, 128, 255]).unwrap();
        assert_eq!(appliquer(&image, 128).pixels, vec![0, 0, 0, 255, 255]);
    }

    #[test]
    fn seuils_extremes() {
        let image = Image::depuis_pixels(3, 1, 1, vec![0, 128, 255]).unwrap();
        assert_eq!(appliquer(&image, 0).pixels, vec![255, 255, 255]);
        assert_eq!(appliquer(&image, 255).pixels, vec![0, 0, 255]);
    }

    #[test]
    fn une_image_couleur_donne_un_pgm_binaire() {
        let image = Image::depuis_pixels(3, 1, 3, vec![255, 0, 0, 0, 255, 0, 0, 0, 255]).unwrap();
        assert_eq!(appliquer(&image, 100).pixels, vec![0, 255, 0]);
    }
}
