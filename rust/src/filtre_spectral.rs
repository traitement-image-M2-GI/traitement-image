//! Filtrage passe-bas / passe-haut dans le domaine spectral.
//!
//! 1. F = FFT(f), après complément à zéro jusqu'à une puissance de 2 ;
//! 2. G(u, v) = F(u, v) · H(D), avec D = √(u² + v²) la distance au centre du spectre recentré ;
//! 3. g = FFT⁻¹(G), recadrée à la taille d'origine.
//!
//! Gabarits H (voir `fourier::Forme`), pour une fréquence de coupure D0 :
//! ```text
//! idéal        : H_pb = 1 si D ≤ D0, 0 sinon           (oscillations autour des contours)
//! Butterworth  : H_pb = 1 / (1 + (D/D0)^(2n))
//! gaussien     : H_pb = exp(-D² / (2·D0²))
//! passe-haut   : H_ph = 1 - H_pb
//! ```
//! D0 se lit en pixels sur le spectre recentré (affiché par le traitement `fft`).
//!
//! Retour dans [0, 255] : le passe-bas reste dans la plage d'origine (saturation des petits
//! dépassements dus aux oscillations). Le passe-haut supprime la composante continue : ses valeurs
//! sont centrées sur 0, positives et négatives, on les normalise (min-max).

use crate::fourier::{self, Forme, Spectre};
use crate::pnm::{self, Image};
use crate::{verifier_arguments, Resultat};

/// Type de filtre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Type {
    PasseBas,
    PasseHaut,
}

impl Type {
    pub fn lire(texte: &str) -> Result<Type, String> {
        match texte {
            "passe-bas" => Ok(Type::PasseBas),
            "passe-haut" => Ok(Type::PasseHaut),
            autre => Err(format!("type de filtre inconnu : « {autre} » (passe-bas ou passe-haut)")),
        }
    }
}

/// Filtre l'image (niveaux de gris) dans le domaine spectral.
pub fn filtrer(image: &Image, type_filtre: Type, forme: Forme, coupure: f64) -> Result<Image, String> {
    if coupure <= 0.0 {
        return Err(format!("la fréquence de coupure doit être > 0 (reçu {coupure})"));
    }
    let mut spectre = Spectre::depuis_image(image)?;
    spectre.filtrer(|fu, fv| {
        let d = fu.hypot(fv);
        match type_filtre {
            Type::PasseBas => forme.passe_bas(d, coupure),
            Type::PasseHaut => forme.passe_haut(d, coupure),
        }
    });

    let valeurs = spectre.image_reconstruite();
    Ok(match type_filtre {
        Type::PasseBas => fourier::saturer(&valeurs, image.largeur, image.hauteur),
        Type::PasseHaut => fourier::normaliser(&valeurs, image.largeur, image.hauteur),
    })
}

/// `traitements filtre-spectral <entrée.pgm> <sortie.pgm> <passe-bas|passe-haut> <ideal|butterworth|gaussien> <coupure> <ordre>`
pub fn executer(args: &[String]) -> Resultat {
    verifier_arguments(
        args,
        &["entrée.pgm", "sortie.pgm", "type (passe-bas|passe-haut)", "forme (ideal|butterworth|gaussien)", "coupure", "ordre"],
    )?;
    let type_filtre = Type::lire(&args[2])?;
    let coupure: f64 = fourier::lire_nombre(&args[4], "coupure")?;
    let ordre: i32 = fourier::lire_nombre(&args[5], "ordre")?;
    let forme = Forme::lire(&args[3], ordre)?;

    let image = pnm::lire(&args[0])?;
    pnm::ecrire(&args[1], &filtrer(&image, type_filtre, forme, coupure)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gris(largeur: usize, hauteur: usize, pixels: Vec<u8>) -> Image {
        Image::depuis_pixels(largeur, hauteur, 1, pixels).unwrap()
    }

    /// Damier 8x8 : 0 et 200 en alternance, c'est la fréquence la plus haute possible (4, 4).
    fn damier() -> Image {
        gris(8, 8, (0..64).map(|i| if (i % 8 + i / 8) % 2 == 0 { 0 } else { 200 }).collect())
    }

    #[test]
    fn passe_bas_garde_une_image_constante() {
        let image = gris(4, 4, vec![90; 16]);
        for forme in [Forme::Ideal, Forme::Butterworth { ordre: 2 }, Forme::Gaussien] {
            assert_eq!(filtrer(&image, Type::PasseBas, forme, 1.0).unwrap(), image);
        }
    }

    #[test]
    fn passe_bas_ideal_efface_le_damier() {
        // Il ne reste que la moyenne : 100 partout
        let resultat = filtrer(&damier(), Type::PasseBas, Forme::Ideal, 3.0).unwrap();
        assert_eq!(resultat.pixels, vec![100; 64]);
    }

    #[test]
    fn passe_haut_ideal_garde_le_damier() {
        // La moyenne est retirée, il reste ±100, normalisé en 0 / 255
        let resultat = filtrer(&damier(), Type::PasseHaut, Forme::Ideal, 3.0).unwrap();
        let attendu: Vec<u8> = damier().pixels.iter().map(|&p| if p == 0 { 0 } else { 255 }).collect();
        assert_eq!(resultat.pixels, attendu);
    }

    #[test]
    fn passe_haut_d_une_image_constante_est_nul() {
        let resultat = filtrer(&gris(4, 4, vec![90; 16]), Type::PasseHaut, Forme::Gaussien, 2.0).unwrap();
        assert_eq!(resultat.pixels, vec![0; 16]);
    }

    #[test]
    fn la_taille_d_origine_est_conservee() {
        let resultat = filtrer(&gris(5, 3, vec![10; 15]), Type::PasseBas, Forme::Gaussien, 4.0).unwrap();
        assert_eq!((resultat.largeur, resultat.hauteur), (5, 3));
    }

    #[test]
    fn coupure_nulle_refusee() {
        assert!(filtrer(&gris(2, 2, vec![0; 4]), Type::PasseBas, Forme::Ideal, 0.0).is_err());
    }
}
