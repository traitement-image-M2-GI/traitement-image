//! Opérateurs morphologiques sur images binaires (cours 07 « Images binaires »,
//! diapos 28 à 42).
//!
//! Un élément structurant glisse sur l'image. Les pixels hors de l'image sont
//! considérés comme du fond (0), donc :
//!   - érosion  : le centre devient objet seulement si TOUS les pixels de B sont objets ;
//!   - dilatation : le centre devient objet si AU MOINS un pixel de B est objet.
//!
//! Élément structurant 4 voisins = croix, 8 voisins = carré 3x3 (diapo 29).
//!
//!   ouverture = érosion puis dilatation  (élimine le bruit, détache les objets)
//!   fermeture = dilatation puis érosion  (bouche les petits trous)
//!   gradient interne      = A - érodée(A)
//!   gradient externe      = dilatée(A) - A
//!   gradient morphologique = dilatée(A) - érodée(A)

use crate::binaire::{est_objet, verifier_binaire};
use crate::pnm::{self, Image};
use crate::{verifier_arguments, Resultat};

// ===========================================================================
// Choix de l'utilisateur
// ===========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Erosion,
    Dilatation,
    Ouverture,
    Fermeture,
    GradientInterne,
    GradientExterne,
    GradientMorphologique,
}

impl Operation {
    pub fn depuis_chaine(texte: &str) -> Result<Operation, String> {
        match texte {
            "erosion" => Ok(Operation::Erosion),
            "dilatation" => Ok(Operation::Dilatation),
            "ouverture" => Ok(Operation::Ouverture),
            "fermeture" => Ok(Operation::Fermeture),
            "gradient_interne" => Ok(Operation::GradientInterne),
            "gradient_externe" => Ok(Operation::GradientExterne),
            "gradient_morphologique" => Ok(Operation::GradientMorphologique),
            autre => Err(format!("opération inconnue : « {autre} »")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Element {
    Quatre,
    Huit,
}

impl Element {
    pub fn depuis_chaine(texte: &str) -> Result<Element, String> {
        match texte {
            "4" => Ok(Element::Quatre),
            "8" => Ok(Element::Huit),
            autre => Err(format!("élément structurant inconnu : « {autre} » (4 ou 8 attendu)")),
        }
    }
}

/// Décalages (dx, dy) de l'élément structurant par rapport au centre.
/// Des `i64` sont utilisés pour pouvoir tester x - 1 quand x vaut 0.
fn voisinage(element: Element) -> &'static [(i64, i64)] {
    match element {
        Element::Quatre => &[(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)],
        Element::Huit => &[
            (-1, -1), (0, -1), (1, -1),
            (-1, 0), (0, 0), (1, 0),
            (-1, 1), (0, 1), (1, 1),
        ],
    }
}

// ===========================================================================
// Opérations de base
// ===========================================================================

fn erosion(image: &Image, element: Element) -> Image {
    let voisins = voisinage(element);
    let mut sortie = Image::nouvelle(image.largeur, image.hauteur, 1);
    for y in 0..image.hauteur {
        for x in 0..image.largeur {
            let tout_objet = voisins
                .iter()
                .all(|&(dx, dy)| est_objet(image, x as i64 + dx, y as i64 + dy));
            sortie.modifier(x, y, 0, if tout_objet { 255 } else { 0 });
        }
    }
    sortie
}

fn dilatation(image: &Image, element: Element) -> Image {
    let voisins = voisinage(element);
    let mut sortie = Image::nouvelle(image.largeur, image.hauteur, 1);
    for y in 0..image.hauteur {
        for x in 0..image.largeur {
            let un_objet = voisins
                .iter()
                .any(|&(dx, dy)| est_objet(image, x as i64 + dx, y as i64 + dy));
            sortie.modifier(x, y, 0, if un_objet { 255 } else { 0 });
        }
    }
    sortie
}

/// Soustraction binaire `a - b` : objet seulement là où a est objet et b est fond.
fn soustraire(a: &Image, b: &Image) -> Image {
    let pixels = a
        .pixels
        .iter()
        .zip(&b.pixels)
        .map(|(&va, &vb)| if va == 255 && vb == 0 { 255 } else { 0 })
        .collect();
    Image::depuis_pixels(a.largeur, a.hauteur, 1, pixels).expect("mêmes dimensions")
}

/// Applique l'opération choisie à une image binaire.
pub fn appliquer(image: &Image, operation: Operation, element: Element) -> Image {
    match operation {
        Operation::Erosion => erosion(image, element),
        Operation::Dilatation => dilatation(image, element),
        Operation::Ouverture => dilatation(&erosion(image, element), element),
        Operation::Fermeture => erosion(&dilatation(image, element), element),
        Operation::GradientInterne => soustraire(image, &erosion(image, element)),
        Operation::GradientExterne => soustraire(&dilatation(image, element), image),
        Operation::GradientMorphologique => {
            soustraire(&dilatation(image, element), &erosion(image, element))
        }
    }
}

/// `traitements morphologie <entrée.pgm> <sortie.pgm> <opération> <4|8>`
pub fn executer(args: &[String]) -> Resultat {
    verifier_arguments(
        args,
        &["entrée.pgm", "sortie.pgm", "opération", "élément (4|8)"],
    )?;
    let image = pnm::lire(&args[0])?;
    verifier_binaire(&image)?;
    let operation = Operation::depuis_chaine(&args[2])?;
    let element = Element::depuis_chaine(&args[3])?;

    pnm::ecrire(&args[1], &appliquer(&image, operation, element))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binaire(largeur: usize, hauteur: usize, pixels: Vec<u8>) -> Image {
        Image::depuis_pixels(largeur, hauteur, 1, pixels).unwrap()
    }

    fn une(largeur: usize, hauteur: usize, positions: &[(usize, usize)]) -> Image {
        let mut pixels = vec![0; largeur * hauteur];
        for &(x, y) in positions {
            pixels[y * largeur + x] = 255;
        }
        binaire(largeur, hauteur, pixels)
    }

    #[test]
    fn erosion_d_un_carre_plein_ne_garde_que_le_centre() {
        let plein = binaire(3, 3, vec![255; 9]);
        let attendu = une(3, 3, &[(1, 1)]);
        assert_eq!(erosion(&plein, Element::Quatre), attendu);
        assert_eq!(erosion(&plein, Element::Huit), attendu);
    }

    #[test]
    fn dilatation_d_un_pixel_donne_la_croix_ou_le_carre() {
        let pixel = une(3, 3, &[(1, 1)]);
        let croix = une(3, 3, &[(1, 0), (0, 1), (1, 1), (2, 1), (1, 2)]);
        assert_eq!(dilatation(&pixel, Element::Quatre), croix);
        assert_eq!(dilatation(&pixel, Element::Huit), binaire(3, 3, vec![255; 9]));
    }

    #[test]
    fn ouverture_elimine_un_pixel_isole() {
        // Bloc 3x3 + pixel isolé en (4,4) : l'ouverture garde le bloc et supprime le bruit.
        let mut objet: Vec<(usize, usize)> = (0..3).flat_map(|y| (0..3).map(move |x| (x, y))).collect();
        objet.push((4, 4));
        let image = une(6, 6, &objet);
        let resultat = appliquer(&image, Operation::Ouverture, Element::Huit);

        assert_eq!(resultat.valeur(4, 4, 0), 0);
        for y in 0..3 {
            for x in 0..3 {
                assert_eq!(resultat.valeur(x, y, 0), 255, "pixel ({x}, {y})");
            }
        }
    }

    #[test]
    fn fermeture_bouche_un_trou() {
        let mut pixels = vec![255; 25]; // carré 5x5 plein
        pixels[12] = 0; // trou au centre (2, 2)
        let image = binaire(5, 5, pixels);
        let resultat = appliquer(&image, Operation::Fermeture, Element::Huit);
        assert_eq!(resultat.valeur(2, 2, 0), 255);
    }

    #[test]
    fn les_trois_gradients_sur_un_pixel_isole() {
        let pixel = une(5, 5, &[(2, 2)]);
        let interne = appliquer(&pixel, Operation::GradientInterne, Element::Huit);
        let externe = appliquer(&pixel, Operation::GradientExterne, Element::Huit);
        let morpho = appliquer(&pixel, Operation::GradientMorphologique, Element::Huit);

        assert_eq!(interne, pixel); // A - erodee(A) = A ici
        assert_eq!(externe.valeur(2, 2, 0), 0); // A retire le centre
        assert_eq!(externe.valeur(1, 1, 0), 255); // 8 voisins
        assert_eq!(morpho.valeur(2, 2, 0), 255); // dilatee - erodee = carre 3x3
        assert_eq!(morpho.valeur(1, 1, 0), 255);
        assert_eq!(morpho.valeur(0, 0, 0), 0);
    }

    #[test]
    fn bords_sont_traites_sans_depassement() {
        let image = une(3, 3, &[(0, 0)]);
        let dilatee = dilatation(&image, Element::Huit);
        assert_eq!(dilatee.valeur(0, 0, 0), 255);
        assert_eq!(dilatee.valeur(1, 1, 0), 255);
        assert_eq!(dilatee.valeur(2, 2, 0), 0);
    }

    #[test]
    fn choix_invalides_donnent_un_message_clair() {
        assert!(Operation::depuis_chaine("moyenne").is_err());
        assert!(Element::depuis_chaine("16").is_err());
    }
}
