//! Réduction de bruit dans le domaine spectral : filtre coupe-bande / notch.
//!
//! Un bruit périodique (trame, lignes régulières…) se voit sur le spectre recentré comme des pics
//! isolés, symétriques par rapport au centre. On les supprime en mettant H à 0 autour d'eux, puis
//! on revient dans le domaine spatial :
//!
//! - `points` (notch) : deux disques de diamètre `largeur`, centrés en (u, v) et (-u, -v).
//!   (u, v) se lit sur le spectre recentré (traitement `fft`) avec l'inspecteur de pixels :
//!   un pic au pixel (x, y) d'un spectre de taille M × N est en u = x - M/2, v = y - N/2.
//!   Le point symétrique est supprimé automatiquement : le résultat reste réel.
//! - `anneau` (coupe-bande) : tout l'anneau de rayon `rayon` et d'épaisseur `largeur` autour du centre,
//!   pour un bruit périodique de même fréquence dans toutes les directions.
//!
//! ```text
//! points : H = 0 si |(fu, fv) - (u, v)| ≤ largeur/2 ou |(fu, fv) + (u, v)| ≤ largeur/2, 1 sinon
//! anneau : H = 0 si |D - rayon| ≤ largeur/2, 1 sinon          (D = √(fu² + fv²))
//! ```
//! Le résultat reste dans la plage d'origine : il est saturé dans [0, 255].

use crate::fourier::{self, Spectre};
use crate::pnm::{self, Image};
use crate::{verifier_arguments, Resultat};

/// Zone du spectre supprimée.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Zone {
    /// Deux disques symétriques, centrés en (u, v) et (-u, -v).
    Points { u: f64, v: f64 },
    /// Anneau de rayon donné autour du centre.
    Anneau { rayon: f64 },
}

/// Gain H du filtre à la fréquence (fu, fv) : 0 dans la zone supprimée, 1 ailleurs.
pub fn gain(zone: Zone, largeur: f64, fu: f64, fv: f64) -> f64 {
    let demi = largeur / 2.0;
    let supprime = match zone {
        Zone::Points { u, v } => (fu - u).hypot(fv - v) <= demi || (fu + u).hypot(fv + v) <= demi,
        Zone::Anneau { rayon } => (fu.hypot(fv) - rayon).abs() <= demi,
    };
    if supprime {
        0.0
    } else {
        1.0
    }
}

/// Supprime la zone `zone` (d'épaisseur ou de diamètre `largeur`) du spectre de l'image.
pub fn filtrer(image: &Image, zone: Zone, largeur: f64) -> Result<Image, String> {
    if largeur <= 0.0 {
        return Err(format!("la largeur doit être > 0 (reçu {largeur})"));
    }
    let mut spectre = Spectre::depuis_image(image)?;
    spectre.filtrer(|fu, fv| gain(zone, largeur, fu, fv));
    Ok(fourier::saturer(&spectre.image_reconstruite(), image.largeur, image.hauteur))
}

/// `traitements coupe-bande <entrée.pgm> <sortie.pgm> <points|anneau> <largeur> <u> <v> <rayon>`
///
/// `u` et `v` ne servent qu'au mode `points`, `rayon` qu'au mode `anneau`.
pub fn executer(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée.pgm", "sortie.pgm", "mode (points|anneau)", "largeur", "u", "v", "rayon"])?;
    let largeur: f64 = fourier::lire_nombre(&args[3], "largeur")?;
    let zone = match args[2].as_str() {
        "points" => Zone::Points { u: fourier::lire_nombre(&args[4], "u")?, v: fourier::lire_nombre(&args[5], "v")? },
        "anneau" => {
            let rayon: f64 = fourier::lire_nombre(&args[6], "rayon")?;
            if rayon < 0.0 {
                return Err(format!("le rayon doit être ≥ 0 (reçu {rayon})").into());
            }
            Zone::Anneau { rayon }
        }
        autre => return Err(format!("mode inconnu : « {autre} » (points ou anneau)").into()),
    };

    let image = pnm::lire(&args[0])?;
    pnm::ecrire(&args[1], &filtrer(&image, zone, largeur)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    /// 8x8 : fond à 128 + trame verticale 100·cos(2π·2x/8) -> pics en (u, v) = (±2, 0).
    fn image_tramee() -> Image {
        let pixels = (0..64)
            .map(|i| {
                let x = (i % 8) as f64;
                (128.0 + 100.0 * (2.0 * PI * 2.0 * x / 8.0).cos()).round() as u8
            })
            .collect();
        Image::depuis_pixels(8, 8, 1, pixels).unwrap()
    }

    #[test]
    fn notch_supprime_la_trame() {
        let resultat = filtrer(&image_tramee(), Zone::Points { u: 2.0, v: 0.0 }, 1.0).unwrap();
        assert_eq!(resultat.pixels, vec![128; 64]);
    }

    #[test]
    fn le_point_symetrique_suffit() {
        let resultat = filtrer(&image_tramee(), Zone::Points { u: -2.0, v: 0.0 }, 1.0).unwrap();
        assert_eq!(resultat.pixels, vec![128; 64]);
    }

    #[test]
    fn anneau_supprime_la_trame() {
        let resultat = filtrer(&image_tramee(), Zone::Anneau { rayon: 2.0 }, 1.0).unwrap();
        assert_eq!(resultat.pixels, vec![128; 64]);
    }

    #[test]
    fn un_notch_ailleurs_ne_change_rien() {
        let image = image_tramee();
        assert_eq!(filtrer(&image, Zone::Points { u: 0.0, v: 3.0 }, 1.0).unwrap(), image);
    }

    #[test]
    fn gain_du_filtre() {
        let zone = Zone::Points { u: 10.0, v: -5.0 };
        assert_eq!(gain(zone, 4.0, 10.0, -5.0), 0.0);
        assert_eq!(gain(zone, 4.0, -11.0, 6.0), 0.0); // près du symétrique (-10, 5)
        assert_eq!(gain(zone, 4.0, 0.0, 0.0), 1.0); // la composante continue est gardée
        assert_eq!(gain(Zone::Anneau { rayon: 10.0 }, 2.0, 0.0, 10.5), 0.0);
        assert_eq!(gain(Zone::Anneau { rayon: 10.0 }, 2.0, 0.0, 12.0), 1.0);
    }

    #[test]
    fn largeur_nulle_refusee() {
        assert!(filtrer(&image_tramee(), Zone::Anneau { rayon: 2.0 }, 0.0).is_err());
    }
}
