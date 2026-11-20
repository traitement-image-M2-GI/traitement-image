//! Conversion couleur -> niveaux de gris (luminance UIT-R BT.601).
//!
//! Même formule que l'interface web (web/app.js) : Y = 0,299 R + 0,587 G + 0,114 B.
//! Les traitements de segmentation travaillent en gris ; ce module est partagé entre eux.
//! Il ne lit aucun fichier : c'est `pnm::lire` qui s'en charge.

use crate::pnm::Image;

/// Luminance d'un pixel RVB, arrondie dans [0, 255].
pub fn luminance(r: u8, g: u8, b: u8) -> u8 {
    (0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64).round() as u8
}

/// Image en niveaux de gris. Une image déjà grise est renvoyée telle quelle (clonée).
pub fn vers_gris(image: &Image) -> Image {
    if !image.est_couleur() {
        return image.clone();
    }
    let pixels = image.pixels.chunks_exact(3).map(|p| luminance(p[0], p[1], p[2])).collect();
    Image::depuis_pixels(image.largeur, image.hauteur, 1, pixels).expect("même nombre de pixels que l'entrée")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn couleurs_primaires() {
        assert_eq!(luminance(255, 0, 0), 76);
        assert_eq!(luminance(0, 255, 0), 150);
        assert_eq!(luminance(0, 0, 255), 29);
    }

    #[test]
    fn vers_gris_garde_les_dimensions() {
        let image = Image::depuis_pixels(2, 1, 3, vec![255, 0, 0, 0, 255, 0]).unwrap();
        let gris = vers_gris(&image);
        assert_eq!((gris.largeur, gris.hauteur, gris.canaux), (2, 1, 1));
        assert_eq!(gris.pixels, vec![76, 150]);
    }

    #[test]
    fn une_image_grise_n_est_pas_modifiee() {
        let image = Image::depuis_pixels(2, 1, 1, vec![10, 20]).unwrap();
        assert_eq!(vers_gris(&image), image);
    }
}
