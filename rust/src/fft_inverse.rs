//! Transformée de Fourier inverse (analyse spectrale).
//!
//! L'interface n'échange que des images 8 bits : un spectre complexe ne peut pas lui être renvoyé
//! sans perte. Ce traitement part donc de l'image, calcule F = FFT(f), puis revient dans le domaine
//! spatial avec la FFT inverse, en gardant tout ou partie de l'information de F :
//!
//! | reconstruction | spectre utilisé avant l'inverse   | ce qu'on observe                                  |
//! |----------------|-----------------------------------|---------------------------------------------------|
//! | `complete`     | F                                 | l'image de départ (vérifie que l'inverse est exact) |
//! | `phase`        | exp(i · arg F)   (module mis à 1) | les contours : la structure est portée par la phase |
//! | `module`       | |F|              (phase mise à 0) | une tache sans rapport avec l'image               |
//!
//! Après l'inverse, le complément à zéro est retiré (image à la taille d'origine).
//! `complete` est saturée dans [0, 255] ; `phase` et `module` sont normalisées (min-max).

use crate::fourier::{self, Complexe, Spectre};
use crate::pnm::{self, Image};
use crate::{verifier_arguments, Resultat};

/// Information du spectre conservée avant la transformée inverse.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reconstruction {
    Complete,
    PhaseSeule,
    ModuleSeul,
}

impl Reconstruction {
    pub fn lire(texte: &str) -> Result<Reconstruction, String> {
        match texte {
            "complete" => Ok(Reconstruction::Complete),
            "phase" => Ok(Reconstruction::PhaseSeule),
            "module" => Ok(Reconstruction::ModuleSeul),
            autre => Err(format!("reconstruction inconnue : « {autre} » (complete, phase ou module)")),
        }
    }
}

/// Image reconstruite (taille d'origine) par transformée inverse.
pub fn reconstruire(spectre: &Spectre, reconstruction: Reconstruction) -> Image {
    let mut spectre = spectre.clone();
    match reconstruction {
        Reconstruction::Complete => {}
        Reconstruction::PhaseSeule => {
            for z in spectre.valeurs.iter_mut() {
                *z = if z.module() < fourier::MODULE_NEGLIGEABLE { Complexe::ZERO } else { Complexe::polaire(1.0, z.phase()) };
            }
        }
        Reconstruction::ModuleSeul => {
            for z in spectre.valeurs.iter_mut() {
                *z = Complexe::reel(z.module());
            }
        }
    }

    let valeurs = spectre.image_reconstruite();
    let (largeur, hauteur) = (spectre.largeur_origine, spectre.hauteur_origine);
    match reconstruction {
        Reconstruction::Complete => fourier::saturer(&valeurs, largeur, hauteur),
        _ => fourier::normaliser(&valeurs, largeur, hauteur),
    }
}

/// `traitements fft-inverse <entrée.pgm> <sortie.pgm> <complete|phase|module>`
pub fn executer(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée.pgm", "sortie.pgm", "reconstruction (complete|phase|module)"])?;
    let reconstruction = Reconstruction::lire(&args[2])?;

    let image = pnm::lire(&args[0])?;
    let spectre = Spectre::depuis_image(&image)?;
    pnm::ecrire(&args[1], &reconstruire(&spectre, reconstruction))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gris(largeur: usize, hauteur: usize, pixels: Vec<u8>) -> Image {
        Image::depuis_pixels(largeur, hauteur, 1, pixels).unwrap()
    }

    #[test]
    fn la_reconstruction_complete_redonne_l_image() {
        // 5x3 : complétée en 8x4, puis recadrée
        let image = gris(5, 3, vec![0, 255, 17, 42, 99, 3, 128, 200, 64, 1, 250, 7, 77, 180, 33]);
        let spectre = Spectre::depuis_image(&image).unwrap();
        assert_eq!(reconstruire(&spectre, Reconstruction::Complete), image);
    }

    #[test]
    fn les_reconstructions_partielles_gardent_la_taille() {
        let image = gris(3, 3, vec![0, 0, 0, 0, 255, 0, 0, 0, 0]);
        let spectre = Spectre::depuis_image(&image).unwrap();
        for r in [Reconstruction::PhaseSeule, Reconstruction::ModuleSeul] {
            let resultat = reconstruire(&spectre, r);
            assert_eq!((resultat.largeur, resultat.hauteur), (3, 3));
        }
    }

    #[test]
    fn phase_seule_d_une_impulsion() {
        // Impulsion en (1, 1) : |F| = 255 partout, donc garder la phase seule redonne l'impulsion
        let image = gris(4, 4, (0..16).map(|i| if i == 5 { 255 } else { 0 }).collect());
        let spectre = Spectre::depuis_image(&image).unwrap();
        assert_eq!(reconstruire(&spectre, Reconstruction::PhaseSeule), image);
    }

    #[test]
    fn reconstruction_inconnue() {
        assert!(Reconstruction::lire("totale").is_err());
    }
}
