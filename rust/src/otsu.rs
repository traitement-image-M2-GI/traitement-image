//! Seuillage automatique d'Otsu (chapitre « Segmentation »).
//!
//! Classe 0 = pixels <= t, classe 1 = pixels > t. On garde le t qui maximise
//! la variance inter-classes w0 * w1 * (m0 - m1)^2.

use crate::gris;
use crate::histogramme;
use crate::pnm::{self, Image};
use crate::seuillage;
use crate::{verifier_arguments, Resultat};
use std::fs;

/// Meilleur seuil : borne supérieure de la classe 0 (objet si la valeur est > seuil).
pub fn seuil(image: &Image) -> u8 {
    let gris = gris::vers_gris(image);
    let h = histogramme::calculer(&gris, 0);
    let n = gris.nb_pixels() as u64;
    let somme_totale: u64 = h.iter().enumerate().map(|(v, &c)| v as u64 * c).sum();

    let mut cumul = 0u64;
    let mut somme0 = 0u64;
    let mut meilleur_t = 0u8;
    let mut meilleure_variance = -1.0f64;

    for t in 0..255u8 {
        cumul += h[t as usize];
        somme0 += t as u64 * h[t as usize];
        let (w0, w1) = (cumul, n - cumul);
        if w0 == 0 || w1 == 0 {
            continue;
        }
        let m0 = somme0 as f64 / w0 as f64;
        let m1 = (somme_totale - somme0) as f64 / w1 as f64;
        let variance = w0 as f64 * w1 as f64 * (m0 - m1).powi(2);
        if variance > meilleure_variance {
            meilleure_variance = variance;
            meilleur_t = t;
        }
    }

    if meilleure_variance < 0.0 {
        h.iter().position(|&c| c > 0).unwrap_or(0) as u8
    } else {
        meilleur_t
    }
}

/// Binarise avec le seuil d'Otsu : objet si la valeur est > t.
pub fn binariser(image: &Image, t: u8) -> Image {
    if t == 255 {
        Image::nouvelle(image.largeur, image.hauteur, 1)
    } else {
        seuillage::appliquer(image, t + 1)
    }
}

fn vers_json(t: u8, nb_pixels: usize) -> String {
    format!("{{\"valeurs\": {{\"seuil\": {t}, \"pixels\": {nb_pixels}}}}}\n")
}

/// `traitements otsu <entrée> <sortie.pnm> <sortie.json>`
pub fn executer(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée", "sortie.pnm", "sortie.json"])?;
    let image = pnm::lire(&args[0])?;
    let t = seuil(&image);
    pnm::ecrire(&args[1], &binariser(&image, t))?;
    fs::write(&args[2], vers_json(t, image.nb_pixels()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gris(pixels: Vec<u8>) -> Image {
        Image::depuis_pixels(pixels.len(), 1, 1, pixels).unwrap()
    }

    #[test]
    fn exemple_fait_a_la_main() {
        let image = gris(vec![0, 0, 0, 100, 100, 100]);
        assert_eq!(seuil(&image), 0);
        assert_eq!(binariser(&image, 0).pixels, vec![0, 0, 0, 255, 255, 255]);
    }

    #[test]
    fn deux_valeurs_plus_elevees() {
        let image = gris(vec![10, 10, 200, 200]);
        assert_eq!(seuil(&image), 10);
        assert_eq!(binariser(&image, 10).pixels, vec![0, 0, 255, 255]);
    }

    #[test]
    fn image_uniforme() {
        let image = gris(vec![42, 42, 42]);
        assert_eq!(seuil(&image), 42);
        assert_eq!(binariser(&image, 42).pixels, vec![0, 0, 0]);
    }

    #[test]
    fn json_contient_le_seuil() {
        assert!(vers_json(10, 4).contains("\"seuil\": 10"));
    }
}
