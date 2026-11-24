//! Changement d'échelle par interpolation (lot « Filtrage spatial », Naomi).
//!
//! On agrandit ou réduit l'image d'un facteur `f` : la nouvelle image fait
//! `arrondi(largeur × f)` × `arrondi(hauteur × f)` pixels.
//!
//! Pour chaque pixel (x', y') de l'image résultat, on cherche sa position dans l'image
//! d'origine (calcul « au centre des pixels ») :
//!   x = (x' + 0,5) / f − 0,5        y = (y' + 0,5) / f − 0,5
//! Cette position tombe en général ENTRE des pixels : l'interpolation dit quelle valeur prendre.
//!
//!   - plus proche voisin : la valeur du pixel le plus proche (rapide, effet « escalier »)
//!   - bilinéaire : moyenne des 4 voisins pondérée par la distance (plus lisse, un peu flou)
//!   - bicubique : 16 voisins (4×4), noyau cubique de Keys (a = −0,5) ; plus net que le bilinéaire
//!
//! Au bord, les coordonnées sont bornées à l'image (on répète le pixel du bord).
//! Une image couleur est traitée canal par canal.

use crate::pnm::{self, Image};
use crate::{verifier_arguments, Resultat};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Methode {
    PlusProcheVoisin,
    Bilineaire,
    Bicubique,
}

impl Methode {
    pub fn depuis_texte(texte: &str) -> Result<Methode, String> {
        match texte {
            "plus-proche-voisin" | "ppv" => Ok(Methode::PlusProcheVoisin),
            "bilineaire" => Ok(Methode::Bilineaire),
            "bicubique" => Ok(Methode::Bicubique),
            autre => Err(format!("méthode inconnue : « {autre} » (plus-proche-voisin, bilineaire ou bicubique)")),
        }
    }
}

/// Valeur du pixel (x, y), canal c, avec des coordonnées bornées à l'image.
fn pixel_borne(image: &Image, x: i64, y: i64, c: usize) -> f64 {
    let x = x.clamp(0, image.largeur as i64 - 1) as usize;
    let y = y.clamp(0, image.hauteur as i64 - 1) as usize;
    image.valeur(x, y, c) as f64
}

/// Noyau cubique de Keys (a = −0,5) : poids d'un voisin situé à la distance `t`.
fn poids_cubique(t: f64) -> f64 {
    let a = -0.5;
    let t = t.abs();
    if t <= 1.0 {
        (a + 2.0) * t * t * t - (a + 3.0) * t * t + 1.0
    } else if t < 2.0 {
        a * t * t * t - 5.0 * a * t * t + 8.0 * a * t - 4.0 * a
    } else {
        0.0
    }
}

/// Valeur interpolée au point (x, y) (coordonnées réelles dans l'image d'origine).
fn interpoler(image: &Image, x: f64, y: f64, c: usize, methode: Methode) -> f64 {
    match methode {
        Methode::PlusProcheVoisin => pixel_borne(image, x.round() as i64, y.round() as i64, c),
        Methode::Bilineaire => {
            let (x0, y0) = (x.floor(), y.floor());
            let (dx, dy) = (x - x0, y - y0); // distances au voisin haut-gauche, dans [0, 1[
            let (x0, y0) = (x0 as i64, y0 as i64);
            let haut = (1.0 - dx) * pixel_borne(image, x0, y0, c) + dx * pixel_borne(image, x0 + 1, y0, c);
            let bas = (1.0 - dx) * pixel_borne(image, x0, y0 + 1, c) + dx * pixel_borne(image, x0 + 1, y0 + 1, c);
            (1.0 - dy) * haut + dy * bas
        }
        Methode::Bicubique => {
            let (x0, y0) = (x.floor(), y.floor());
            let (dx, dy) = (x - x0, y - y0);
            let (x0, y0) = (x0 as i64, y0 as i64);
            let mut somme = 0.0;
            for j in -1..=2 {
                let wy = poids_cubique(dy - j as f64);
                for i in -1..=2 {
                    let wx = poids_cubique(dx - i as f64);
                    somme += wx * wy * pixel_borne(image, x0 + i, y0 + j, c);
                }
            }
            somme
        }
    }
}

/// Redimensionne l'image d'un facteur `facteur` (> 0) avec la méthode choisie.
pub fn redimensionner(image: &Image, facteur: f64, methode: Methode) -> Image {
    let largeur = ((image.largeur as f64 * facteur).round() as usize).max(1);
    let hauteur = ((image.hauteur as f64 * facteur).round() as usize).max(1);
    // Facteurs réels par axe (l'arrondi de la taille peut les changer légèrement)
    let fx = largeur as f64 / image.largeur as f64;
    let fy = hauteur as f64 / image.hauteur as f64;

    let mut sortie = Image::nouvelle(largeur, hauteur, image.canaux);
    for yp in 0..hauteur {
        let y = (yp as f64 + 0.5) / fy - 0.5;
        for xp in 0..largeur {
            let x = (xp as f64 + 0.5) / fx - 0.5;
            for c in 0..image.canaux {
                // Le bicubique peut dépasser [0, 255] : on ramène dans la plage
                let v = interpoler(image, x, y, c, methode).round().clamp(0.0, 255.0);
                sortie.modifier(xp, yp, c, v as u8);
            }
        }
    }
    sortie
}

/// `traitements interpolation <entrée> <sortie> <plus-proche-voisin|bilineaire|bicubique> <facteur>`
pub fn executer(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée", "sortie", "méthode", "facteur"])?;
    let image = pnm::lire(&args[0])?;
    let methode = Methode::depuis_texte(&args[2])?;
    let facteur: f64 = args[3].parse().map_err(|_| format!("facteur invalide : « {} » (nombre attendu)", args[3]))?;
    if !(facteur > 0.0 && facteur <= 16.0) {
        return Err(format!("le facteur doit être dans ]0, 16] (reçu {facteur})").into());
    }
    let largeur = (image.largeur as f64 * facteur).round();
    let hauteur = (image.hauteur as f64 * facteur).round();
    if largeur * hauteur > 50_000_000.0 {
        return Err(format!("image résultat trop grande ({largeur} x {hauteur})").into());
    }
    pnm::ecrire(&args[1], &redimensionner(&image, facteur, methode))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gris(largeur: usize, hauteur: usize, pixels: Vec<u8>) -> Image {
        Image::depuis_pixels(largeur, hauteur, 1, pixels).unwrap()
    }

    #[test]
    fn plus_proche_voisin_double_chaque_pixel() {
        // Facteur 2 : chaque pixel devient un bloc 2×2
        let image = gris(2, 1, vec![10, 200]);
        let r = redimensionner(&image, 2.0, Methode::PlusProcheVoisin);
        assert_eq!((r.largeur, r.hauteur), (4, 2));
        assert_eq!(r.pixels, vec![10, 10, 200, 200, 10, 10, 200, 200]);
    }

    #[test]
    fn bilineaire_calcule_a_la_main() {
        // Ligne [0, 100], facteur 2. Positions d'origine : x = (x'+0,5)/2 − 0,5
        //   x'=0 -> −0,25 (borné : 0)      x'=1 -> 0,25 -> 0,75×0 + 0,25×100 = 25
        //   x'=2 -> 0,75 -> 75              x'=3 -> 1,25 (borné : 100)
        let image = gris(2, 1, vec![0, 100]);
        let r = redimensionner(&image, 2.0, Methode::Bilineaire);
        assert_eq!(&r.pixels[0..4], &[0, 25, 75, 100]);
    }

    #[test]
    fn image_uniforme_reste_uniforme() {
        let image = gris(3, 3, vec![77; 9]);
        for m in [Methode::PlusProcheVoisin, Methode::Bilineaire, Methode::Bicubique] {
            let r = redimensionner(&image, 1.7, m);
            assert!(r.pixels.iter().all(|&v| v == 77), "{m:?}");
        }
    }

    #[test]
    fn facteur_un_ne_change_rien() {
        let image = gris(3, 2, vec![0, 50, 100, 150, 200, 250]);
        for m in [Methode::PlusProcheVoisin, Methode::Bilineaire, Methode::Bicubique] {
            assert_eq!(redimensionner(&image, 1.0, m), image, "{m:?}");
        }
    }

    #[test]
    fn reduction_et_couleur() {
        let image = Image::depuis_pixels(4, 4, 3, vec![120; 48]).unwrap();
        let r = redimensionner(&image, 0.5, Methode::Bicubique);
        assert_eq!((r.largeur, r.hauteur, r.canaux), (2, 2, 3));
    }

    #[test]
    fn poids_cubique_valeurs_connues() {
        assert_eq!(poids_cubique(0.0), 1.0);
        assert_eq!(poids_cubique(1.0), 0.0);
        assert_eq!(poids_cubique(2.0), 0.0);
        // Somme des 4 poids = 1 quelle que soit la position
        let d = 0.3;
        let s: f64 = (-1..=2).map(|i| poids_cubique(d - i as f64)).sum();
        assert!((s - 1.0).abs() < 1e-12);
    }
}
