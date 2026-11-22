//! Transformée de Fourier 2D d'une image et affichage de ses composantes (analyse spectrale).
//!
//! L'image est complétée par des zéros jusqu'à une puissance de 2 dans chaque direction, puis on
//! calcule F(u, v) (voir `fourier.rs`). Le résultat est complexe : on en affiche une composante,
//! ramenée dans [0, 255] (règle 4 de DEVELOPPEMENT.md) :
//!
//! | composante   | valeur affichée                    | mise dans [0, 255]            |
//! |--------------|------------------------------------|-------------------------------|
//! | `module`     | log(1 + |F|)                       | normalisation min-max         |
//! | `phase`      | arg F, dans [-π, π]                | -π -> 0, 0 -> 128, π -> 255   |
//! | `reelle`     | signe(Re F) · log(1 + |Re F|)      | normalisation min-max         |
//! | `imaginaire` | signe(Im F) · log(1 + |Im F|)      | normalisation min-max         |
//!
//! Le logarithme est indispensable : la composante continue F(0, 0) (somme de tous les pixels)
//! est des milliers de fois plus grande que le reste, qui serait noir sans lui. Pour les parties
//! réelle et imaginaire, qui peuvent être négatives, on applique la même compression en gardant le signe.
//!
//! Recentrage (inversion des quadrants) : la fréquence nulle est placée au centre de l'image.
//! L'image résultat a la taille COMPLÉTÉE (puissances de 2) : c'est la taille du spectre.

use crate::fourier::{self, Spectre};
use crate::pnm::{self, Image};
use crate::{verifier_arguments, Resultat};
use std::f64::consts::PI;

/// Composante du spectre à afficher.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Composante {
    Module,
    Phase,
    Reelle,
    Imaginaire,
}

impl Composante {
    pub fn lire(texte: &str) -> Result<Composante, String> {
        match texte {
            "module" => Ok(Composante::Module),
            "phase" => Ok(Composante::Phase),
            "reelle" => Ok(Composante::Reelle),
            "imaginaire" => Ok(Composante::Imaginaire),
            autre => Err(format!("composante inconnue : « {autre} » (module, phase, reelle ou imaginaire)")),
        }
    }
}

/// log(1 + |v|) avec le signe de v.
fn log_signe(v: f64) -> f64 {
    v.signum() * v.abs().ln_1p()
}

/// Image (taille complétée) de la composante demandée du spectre, recentrée ou non.
pub fn afficher(spectre: &Spectre, composante: Composante, recentrer: bool) -> Image {
    let valeurs: Vec<f64> = spectre
        .valeurs
        .iter()
        .map(|&z| match composante {
            Composante::Module => z.module().ln_1p(),
            Composante::Phase => (z.phase() + PI) / (2.0 * PI) * 255.0,
            Composante::Reelle => log_signe(z.re),
            Composante::Imaginaire => log_signe(z.im),
        })
        .collect();

    let (largeur, hauteur) = (spectre.largeur, spectre.hauteur);
    let valeurs = if recentrer { fourier::recentrer(&valeurs, largeur, hauteur) } else { valeurs };

    match composante {
        // la phase est déjà dans [0, 255] avec une échelle fixe : une normalisation la fausserait
        Composante::Phase => fourier::saturer(&valeurs, largeur, hauteur),
        _ => fourier::normaliser(&valeurs, largeur, hauteur),
    }
}

/// `traitements fft <entrée.pgm> <sortie.pgm> <module|phase|reelle|imaginaire> <recentrer 1|0>`
pub fn executer(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée.pgm", "sortie.pgm", "composante (module|phase|reelle|imaginaire)", "recentrer (1|0)"])?;
    let composante = Composante::lire(&args[2])?;
    let recentrer = fourier::lire_booleen(&args[3], "recentrer")?;

    let image = pnm::lire(&args[0])?;
    let spectre = Spectre::depuis_image(&image)?;
    pnm::ecrire(&args[1], &afficher(&spectre, composante, recentrer))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spectre(largeur: usize, hauteur: usize, pixels: Vec<u8>) -> Spectre {
        Spectre::depuis_image(&Image::depuis_pixels(largeur, hauteur, 1, pixels).unwrap()).unwrap()
    }

    #[test]
    fn image_constante_un_seul_point_blanc() {
        // Image constante : seul F(0, 0) est non nul
        let s = spectre(4, 4, vec![50; 16]);
        let brut = afficher(&s, Composante::Module, false);
        assert_eq!(brut.valeur(0, 0, 0), 255);
        assert_eq!(brut.pixels.iter().filter(|&&p| p != 0).count(), 1);
        // recentré : le point blanc est au centre (largeur/2, hauteur/2)
        let recentre = afficher(&s, Composante::Module, true);
        assert_eq!(recentre.valeur(2, 2, 0), 255);
    }

    #[test]
    fn le_spectre_a_la_taille_completee() {
        let s = spectre(5, 3, vec![10; 15]);
        let image = afficher(&s, Composante::Module, true);
        assert_eq!((image.largeur, image.hauteur), (8, 4));
    }

    #[test]
    fn phase_echelle_fixe() {
        // [0, 10] : F(0) = 10 (phase 0 -> 128), F(1) = -10 (phase π -> 255)
        let s = spectre(2, 1, vec![0, 10]);
        assert_eq!(afficher(&s, Composante::Phase, false).pixels, vec![128, 255]);
    }

    #[test]
    fn parties_reelle_et_imaginaire() {
        // [0, 10] : spectre réel, la partie imaginaire est nulle partout -> image noire
        let s = spectre(2, 1, vec![0, 10]);
        assert_eq!(afficher(&s, Composante::Reelle, false).pixels, vec![255, 0]);
        assert_eq!(afficher(&s, Composante::Imaginaire, false).pixels, vec![0, 0]);
    }

    #[test]
    fn composante_inconnue() {
        assert!(Composante::lire("spectre").is_err());
    }
}
