//! Codage de Freeman d'un contour (cours 07 « Images binaires », diapos 8 et 9).
//!
//! Le code de Freeman décrit un contour par une suite de directions. La numérotation
//! 8-connexité du cours est :
//! ```text
//! 3 2 1
//! 4 • 0
//! 5 6 7
//! ```
//! (0 = Est, 1 = Sud-Est, 2 = Sud, … 7 = Nord-Est, l'origine étant en haut à gauche.)
//!
//! Suivi du contour (voisin de Moore) :
//!   1. partir du premier pixel objet en parcourant l'image ligne par ligne ;
//!   2. le pixel à l'ouest sert de « retour en arrière » ;
//!   3. examiner les voisins dans l'ordre horaire en commençant juste après le retour ;
//!   4. dès qu'un objet est trouvé, mémoriser son code, avancer et recommencer ;
//!   5. s'arrêter en revenant au départ (ou si aucun voisin n'est objet).
//!
//! Un seul contour est suivi : celui du premier pixel objet. Sortie : l'image du
//! contour parcouru et un JSON contenant la chaîne de codes.

use crate::binaire::{est_objet, verifier_binaire, Connexite};
use crate::pnm::{self, Image};
use crate::{verifier_arguments, Resultat};
use std::fs;

/// Résultat du suivi de contour.
pub struct Contour {
    pub codes: Vec<u8>,
    pub depart: (usize, usize),
    pub pixels: Vec<(usize, usize)>,
}

/// Suit le contour commençant au premier pixel objet (parcours haut → bas, gauche → droite).
pub fn suivre(image: &Image, connexite: Connexite) -> Result<Contour, String> {
    let mut depart = None;
    'recherche: for y in 0..image.hauteur {
        for x in 0..image.largeur {
            if image.valeur(x, y, 0) == 255 {
                depart = Some((x, y));
                break 'recherche;
            }
        }
    }
    let depart = depart.ok_or_else(|| "aucun pixel objet (255) trouvé dans l'image".to_string())?;

    let directions = connexite.directions_freeman();
    let n = directions.len();
    let mut p = depart;
    // Le premier pixel objet de la ligne a forcément du fond à sa gauche.
    let mut retour = (depart.0 as i64 - 1, depart.1 as i64);
    let mut codes = Vec::new();
    let mut pixels = vec![depart];
    // Garde-fou : on ne peut pas faire plus de 8 pas par pixel sans tourner en rond.
    let limite = image.nb_pixels() * 8 + 8;

    for _ in 0..limite {
        let dx_retour = retour.0 - p.0 as i64;
        let dy_retour = retour.1 - p.1 as i64;
        let index_retour = directions
            .iter()
            .position(|&(_, dx, dy)| dx == dx_retour && dy == dy_retour)
            .unwrap_or(0);
        let debut = (index_retour + 1) % n; // on cherche juste après le retour, sens horaire

        let mut trouve = None;
        for k in 0..n {
            let index = (debut + k) % n;
            let (_, dx, dy) = directions[index];
            if est_objet(image, p.0 as i64 + dx, p.1 as i64 + dy) {
                trouve = Some((index, k));
                break;
            }
        }
        let Some((index, k)) = trouve else { break }; // pixel isolé : fin du contour

        let (code, dx, dy) = directions[index];
        let suivant = ((p.0 as i64 + dx) as usize, (p.1 as i64 + dy) as usize);
        codes.push(code);

        // Le dernier voisin examiné avant l'objet était du fond : c'est le nouveau retour.
        let index_precedent = (debut + k + n - 1) % n;
        let (_, dx_prec, dy_prec) = directions[index_precedent];
        retour = (p.0 as i64 + dx_prec, p.1 as i64 + dy_prec);
        p = suivant;

        if p == depart || pixels.contains(&p) {
            break; // contour fermé (ou contour qui se touche)
        }
        pixels.push(p);
    }

    Ok(Contour { codes, depart, pixels })
}

/// Image binaire ne contenant que les pixels du contour parcouru.
fn image_contour(image: &Image, contour: &Contour) -> Image {
    let mut sortie = Image::nouvelle(image.largeur, image.hauteur, 1);
    for &(x, y) in &contour.pixels {
        sortie.modifier(x, y, 0, 255);
    }
    sortie
}

/// JSON de sortie : la chaîne de codes est affichée dans le tableau « valeurs ».
fn vers_json(contour: &Contour, connexite: Connexite) -> String {
    let chaine: String = contour.codes.iter().map(|&c| (b'0' + c) as char).collect();
    let nombre = if connexite == Connexite::Huit { 8 } else { 4 };
    format!(
        "{{\n\"valeurs\": {{\"connexite\": {nombre}, \"depart_x\": {}, \"depart_y\": {}, \"directions\": \"{chaine}\", \"nb_pas\": {}}}\n}}\n",
        contour.depart.0,
        contour.depart.1,
        contour.codes.len()
    )
}

/// `traitements freeman <entrée.pgm> <image_sortie.pgm> <json_sortie.json> <4|8>`
pub fn executer(args: &[String]) -> Resultat {
    verifier_arguments(
        args,
        &["entrée.pgm", "sortie.pgm", "sortie.json", "connexité (4|8)"],
    )?;
    let image = pnm::lire(&args[0])?;
    verifier_binaire(&image)?;
    let connexite = Connexite::depuis_chaine(&args[3])?;
    let contour = suivre(&image, connexite)?;

    pnm::ecrire(&args[1], &image_contour(&image, &contour))?;
    fs::write(&args[2], vers_json(&contour, connexite))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binaire(largeur: usize, hauteur: usize, pixels: Vec<u8>) -> Image {
        Image::depuis_pixels(largeur, hauteur, 1, pixels).unwrap()
    }

    #[test]
    fn carre_plein_donne_le_tour_complet() {
        // 0,0 -> 2,0 -> 2,2 -> 0,2 -> 0,0 : 0 0 2 2 4 4 6 6
        let image = binaire(3, 3, vec![255; 9]);
        let contour = suivre(&image, Connexite::Huit).unwrap();
        assert_eq!(contour.depart, (0, 0));
        assert_eq!(contour.codes, vec![0, 0, 2, 2, 4, 4, 6, 6]);
        assert_eq!(contour.pixels.len(), 8);
    }

    #[test]
    fn pixel_isole_n_a_aucune_direction() {
        let image = binaire(3, 3, vec![0, 0, 0, 0, 255, 0, 0, 0, 0]);
        let contour = suivre(&image, Connexite::Huit).unwrap();
        assert!(contour.codes.is_empty());
        assert_eq!(contour.pixels, vec![(1, 1)]);
    }

    #[test]
    fn image_vide_est_refusee() {
        let image = binaire(2, 2, vec![0; 4]);
        assert!(suivre(&image, Connexite::Huit).is_err());
    }

    #[test]
    fn connexite_4_utilise_les_codes_du_cours() {
        // 0 = Est, 1 = Nord, 2 = Ouest, 3 = Sud (diapo 8)
        let image = binaire(2, 1, vec![255, 255]);
        let contour = suivre(&image, Connexite::Quatre).unwrap();
        assert_eq!(contour.codes, vec![0, 2]);
    }

    #[test]
    fn json_contient_la_chaine_et_le_depart() {
        let image = binaire(2, 1, vec![255, 255]);
        let contour = suivre(&image, Connexite::Huit).unwrap();
        let json = vers_json(&contour, Connexite::Huit);
        assert!(json.contains("\"directions\": \"0"));
        assert!(json.contains("\"depart_x\": 0"));
        assert!(json.contains("\"nb_pas\": 2"));
    }
}
