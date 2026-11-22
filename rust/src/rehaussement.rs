//! Rehaussement de contraste dans le domaine spectral : image + k × (image filtrée passe-haut).
//!
//! Le passe-haut isole les détails (contours, textures). En les rajoutant à l'image, avec un poids
//! k, on accentue les détails sans perdre les zones uniformes (que le passe-haut seul efface).
//! La transformée de Fourier étant linéaire, on fait l'addition directement dans le domaine spectral :
//! ```text
//! G = F + k · H_ph · F = F · (1 + k · H_ph)       (H_ph = gabarit passe-haut, voir `fourier::Forme`)
//! g = FFT⁻¹(G), recadrée à la taille d'origine, puis saturée dans [0, 255]
//! ```
//! k = 0 redonne l'image de départ. Les valeurs qui sortent de [0, 255] sont saturées, comme
//! pour une addition d'images (et non normalisées : les niveaux de gris de l'image sont conservés).

use crate::fourier::{self, Forme, Spectre};
use crate::pnm::{self, Image};
use crate::{verifier_arguments, Resultat};

/// Rehausse l'image (niveaux de gris) : g = f + k · passe_haut(f).
pub fn rehausser(image: &Image, forme: Forme, coupure: f64, k: f64) -> Result<Image, String> {
    if coupure <= 0.0 {
        return Err(format!("la fréquence de coupure doit être > 0 (reçu {coupure})"));
    }
    if k < 0.0 {
        return Err(format!("le coefficient k doit être ≥ 0 (reçu {k})"));
    }
    let mut spectre = Spectre::depuis_image(image)?;
    spectre.filtrer(|fu, fv| 1.0 + k * forme.passe_haut(fu.hypot(fv), coupure));
    Ok(fourier::saturer(&spectre.image_reconstruite(), image.largeur, image.hauteur))
}

/// `traitements rehaussement-spectral <entrée.pgm> <sortie.pgm> <ideal|butterworth|gaussien> <coupure> <ordre> <k>`
pub fn executer(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée.pgm", "sortie.pgm", "forme (ideal|butterworth|gaussien)", "coupure", "ordre", "k"])?;
    let coupure: f64 = fourier::lire_nombre(&args[3], "coupure")?;
    let ordre: i32 = fourier::lire_nombre(&args[4], "ordre")?;
    let k: f64 = fourier::lire_nombre(&args[5], "k")?;
    let forme = Forme::lire(&args[2], ordre)?;

    let image = pnm::lire(&args[0])?;
    pnm::ecrire(&args[1], &rehausser(&image, forme, coupure, k)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gris(largeur: usize, hauteur: usize, pixels: Vec<u8>) -> Image {
        Image::depuis_pixels(largeur, hauteur, 1, pixels).unwrap()
    }

    #[test]
    fn k_nul_redonne_l_image() {
        let image = gris(3, 2, vec![10, 200, 30, 40, 50, 255]);
        assert_eq!(rehausser(&image, Forme::Gaussien, 2.0, 0.0).unwrap(), image);
    }

    #[test]
    fn une_zone_uniforme_ne_change_pas() {
        let image = gris(4, 4, vec![77; 16]);
        assert_eq!(rehausser(&image, Forme::Ideal, 1.0, 3.0).unwrap(), image);
    }

    #[test]
    fn le_contraste_du_damier_augmente() {
        // Damier 4x4 de 100 / 140 : fréquence (2, 2), entièrement gardée par le passe-haut idéal.
        // k = 1 double l'écart à la moyenne (120 ± 20 -> 120 ± 40) : 80 / 160
        let damier = |a: u8, b: u8| gris(4, 4, (0..16).map(|i| if (i % 4 + i / 4) % 2 == 0 { a } else { b }).collect());
        assert_eq!(rehausser(&damier(100, 140), Forme::Ideal, 1.0, 1.0).unwrap(), damier(80, 160));
    }

    #[test]
    fn parametres_invalides() {
        let image = gris(2, 2, vec![0; 4]);
        assert!(rehausser(&image, Forme::Ideal, 0.0, 1.0).is_err());
        assert!(rehausser(&image, Forme::Ideal, 1.0, -1.0).is_err());
    }
}
