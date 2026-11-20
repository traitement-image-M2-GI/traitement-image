//! Codage de Freeman des contours (cours 07 « Images binaires », diapos 8 et 9).
//!
//! Le code de Freeman décrit un contour par une suite de directions. La numérotation
//! 8-connexité du cours est :
//! ```text
//! 3 2 1
//! 4 • 0
//! 5 6 7
//! ```
//! (0 = Est, 1 = Sud-Est, 2 = Sud, … 7 = Nord-Est, l'origine étant en haut à gauche.)
//! En 4-connexité, les codes du cours sont 0 = Est, 1 = Nord, 2 = Ouest, 3 = Sud.
//!
//! Conformément au cours (diapo 8), on part du pixel en haut à gauche de chaque
//! contour et on tourne dans le sens des aiguilles d'une montre. Toutes les
//! composantes de l'image sont traitées, pas seulement la première.
//!
//! Suivi du contour (voisin de Moore avec critère d'arrêt de Jacob) :
//!   1. partir du premier pixel objet encore non traité (parcours ligne par ligne) ;
//!   2. le pixel à l'ouest sert de « retour en arrière » initial ;
//!   3. examiner les voisins dans l'ordre horaire en commençant juste après le retour ;
//!   4. dès qu'un objet est trouvé, mémoriser son code, avancer et recommencer ;
//!   5. s'arrêter quand on revient au départ **par le même côté que lors du départ**
//!      (critère de Jacob), ou si aucun voisin n'est objet (contour ouvert).
//!
//! Contrairement à un arrêt sur « pixel déjà visité », le critère de Jacob ne coupe
//! pas une branche ou une boucle secondaire lors d'une forme ramifiée.
//!
//! Sorties : l'image de tous les contours parcourus et un JSON contenant une chaîne
//! de codes par contour.

use crate::binaire::{est_objet, verifier_binaire, Connexite};
use crate::pnm::{self, Image};
use crate::{verifier_arguments, Resultat};
use std::fs;

/// Résultat du suivi d'un contour.
pub struct Contour {
    pub codes: Vec<u8>,
    pub depart: (usize, usize),
    pub pixels: Vec<(usize, usize)>,
}

/// Suit un seul contour à partir du pixel `depart`, dans le sens horaire.
fn tracer(image: &Image, connexite: Connexite, depart: (usize, usize)) -> Contour {
    let directions = connexite.directions_freeman();
    let n = directions.len();
    // Le pixel objet le plus en haut à gauche a du fond à sa gauche : c'est le retour initial.
    let retour_initial: (i64, i64) = (depart.0 as i64 - 1, depart.1 as i64);

    let mut p = depart;
    let mut retour = retour_initial;
    let mut codes = Vec::new();
    let mut pixels = vec![depart];
    // Garde-fou : 8 pas par pixel suffisent pour un contour simple.
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
        let Some((index, k)) = trouve else { break }; // pixel isolé ou fin de contour ouvert

        let (code, dx, dy) = directions[index];
        let suivant = ((p.0 as i64 + dx) as usize, (p.1 as i64 + dy) as usize);
        codes.push(code);

        // Le dernier voisin examiné avant l'objet était du fond : c'est le nouveau retour.
        let index_precedent = (debut + k + n - 1) % n;
        let (_, dx_prec, dy_prec) = directions[index_precedent];
        retour = (p.0 as i64 + dx_prec, p.1 as i64 + dy_prec);
        p = suivant;

        // Critère d'arrêt :
        //  - en 8-connexité, critère de Jacob : revenir au départ par le même côté
        //    que lors du départ. Un simple « pixel déjà visité » couperait une
        //    boucle secondaire ou une branche.
        //  - en 4-connexité (peu utilisée, diapo 8), le retour peut sortir des
        //    4 directions autorisées : on s'arrête dès qu'on revient au départ.
        let ferme = match connexite {
            Connexite::Huit => p == depart && retour == retour_initial,
            Connexite::Quatre => p == depart,
        };
        if ferme {
            break;
        }
        pixels.push(p);
    }

    Contour { codes, depart, pixels }
}

/// Marque tous les pixels de la composante contenant `depart` pour ne pas la reparcourir.
fn marquer_composante(
    image: &Image,
    connexite: Connexite,
    depart: (usize, usize),
    visites: &mut [bool],
) {
    let voisins = connexite.voisins();
    let mut pile = vec![depart];
    visites[depart.1 * image.largeur + depart.0] = true;

    while let Some((x, y)) = pile.pop() {
        for &(dx, dy) in voisins {
            let nx = x as i64 + dx;
            let ny = y as i64 + dy;
            if nx < 0 || ny < 0 || nx >= image.largeur as i64 || ny >= image.hauteur as i64 {
                continue;
            }
            let i = ny as usize * image.largeur + nx as usize;
            if !visites[i] && image.valeur(nx as usize, ny as usize, 0) == 255 {
                visites[i] = true;
                pile.push((nx as usize, ny as usize));
            }
        }
    }
}

/// Suit **tous** les contours de l'image, un par composante non encore traitée.
pub fn suivre_tous(image: &Image, connexite: Connexite) -> Result<Vec<Contour>, String> {
    let mut visites = vec![false; image.nb_pixels()];
    let mut contours = Vec::new();

    loop {
        let mut depart = None;
        'recherche: for y in 0..image.hauteur {
            for x in 0..image.largeur {
                let i = y * image.largeur + x;
                if image.valeur(x, y, 0) == 255 && !visites[i] {
                    depart = Some((x, y));
                    break 'recherche;
                }
            }
        }
        let Some(depart) = depart else { break };

        let contour = tracer(image, connexite, depart);
        // Le suivi ne passe pas forcément par tous les pixels (le centre d'un carré
        // plein, par exemple) : on marque la composante entière pour ne pas la
        // reprendre comme un nouveau contour.
        marquer_composante(image, connexite, depart, &mut visites);
        contours.push(contour);
    }

    if contours.is_empty() {
        Err("aucun pixel objet (255) trouvé dans l'image".to_string())
    } else {
        Ok(contours)
    }
}

/// Image binaire ne contenant que les pixels des contours parcourus.
fn image_contours(image: &Image, contours: &[Contour]) -> Image {
    let mut sortie = Image::nouvelle(image.largeur, image.hauteur, 1);
    for contour in contours {
        for &(x, y) in &contour.pixels {
            sortie.modifier(x, y, 0, 255);
        }
    }
    sortie
}

/// JSON de sortie : une chaîne de codes, un départ et un nombre de pas par contour.
fn vers_json(contours: &[Contour], connexite: Connexite) -> String {
    let connexion = if connexite == Connexite::Huit { 8 } else { 4 };
    let mut champs = vec![
        format!("\"nb_contours\": {}", contours.len()),
        format!("\"connexite\": {connexion}"),
    ];
    for (i, contour) in contours.iter().enumerate() {
        let n = i + 1;
        let chaine: String = contour.codes.iter().map(|&c| (b'0' + c) as char).collect();
        champs.push(format!("\"depart_{n}\": \"{},{}\"", contour.depart.0, contour.depart.1));
        champs.push(format!("\"directions_{n}\": \"{chaine}\""));
        champs.push(format!("\"nb_pas_{n}\": {}", contour.codes.len()));
    }
    format!("{{\n  \"valeurs\": {{{}}}\n}}\n", champs.join(", "))
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
    let contours = suivre_tous(&image, connexite)?;

    pnm::ecrire(&args[1], &image_contours(&image, &contours))?;
    fs::write(&args[2], vers_json(&contours, connexite))?;
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
    fn carre_plein_donne_le_tour_complet() {
        let image = binaire(3, 3, vec![255; 9]);
        let contours = suivre_tous(&image, Connexite::Huit).unwrap();
        assert_eq!(contours.len(), 1);
        assert_eq!(contours[0].depart, (0, 0));
        // 0,0 -> 2,0 -> 2,2 -> 0,2 -> 0,0 : 0 0 2 2 4 4 6 6
        assert_eq!(contours[0].codes, vec![0, 0, 2, 2, 4, 4, 6, 6]);
        assert_eq!(contours[0].pixels.len(), 8);
    }

    #[test]
    fn pixel_isole_n_a_aucune_direction() {
        let image = une(3, 3, &[(1, 1)]);
        let contours = suivre_tous(&image, Connexite::Huit).unwrap();
        assert_eq!(contours.len(), 1);
        assert!(contours[0].codes.is_empty());
        assert_eq!(contours[0].pixels, vec![(1, 1)]);
    }

    #[test]
    fn image_vide_est_refusee() {
        let image = binaire(2, 2, vec![0; 4]);
        assert!(suivre_tous(&image, Connexite::Huit).is_err());
    }

    #[test]
    fn connexite_4_utilise_les_codes_du_cours() {
        // 0 = Est, 1 = Nord, 2 = Ouest, 3 = Sud (diapo 8)
        let image = une(2, 1, &[(0, 0), (1, 0)]);
        let contours = suivre_tous(&image, Connexite::Quatre).unwrap();
        assert_eq!(contours[0].codes, vec![0, 2]);
    }

    #[test]
    fn deux_objets_separes_donnent_deux_contours() {
        let image = une(7, 3, &[(0, 0), (1, 0), (0, 1), (1, 1), (5, 1), (6, 1), (5, 2), (6, 2)]);
        let contours = suivre_tous(&image, Connexite::Huit).unwrap();

        assert_eq!(contours.len(), 2);
        assert_eq!(contours[0].depart, (0, 0));
        assert_eq!(contours[1].depart, (5, 1));
        assert_eq!(contours[0].codes, vec![0, 2, 4, 6]);
        assert_eq!(contours[1].codes, vec![0, 2, 4, 6]);

        let sortie = image_contours(&image, &contours);
        assert_eq!(sortie.pixels.iter().filter(|&&v| v == 255).count(), 8);
    }

    #[test]
    fn json_contient_une_chaine_par_contour() {
        let image = une(7, 3, &[(0, 0), (1, 0), (0, 1), (1, 1), (5, 1), (6, 1), (5, 2), (6, 2)]);
        let contours = suivre_tous(&image, Connexite::Huit).unwrap();
        let json = vers_json(&contours, Connexite::Huit);
        assert!(json.contains("\"nb_contours\": 2"));
        assert!(json.contains("\"connexite\": 8"));
        assert!(json.contains("\"depart_1\": \"0,0\""));
        assert!(json.contains("\"directions_1\": \"0246\""));
        assert!(json.contains("\"directions_2\": \"0246\""));
        assert!(json.contains("\"nb_pas_1\": 4"));
    }

    #[test]
    fn forme_en_huit_ne_s_arrete_pas_au_pixel_deja_visite() {
        // Deux carres 3x3 partageant le pixel (2,2) : une seule composante 8-connexe.
        // L'ancien arret « pixels.contains(&p) » coupait le suivi au carrefour.
        let mut positions = Vec::new();
        for y in 0..3 {
            for x in 0..3 {
                positions.push((x, y));
            }
        }
        for y in 2..5 {
            for x in 2..5 {
                positions.push((x, y));
            }
        }
        let image = une(5, 5, &positions);
        let contours = suivre_tous(&image, Connexite::Huit).unwrap();

        assert_eq!(contours.len(), 1);
        let contour = &contours[0];
        assert!(contour.pixels.contains(&(4, 4)), "le second carre doit etre parcouru");
        assert!(contour.codes.len() > 8, "la boucle secondaire doit etre parcourue");
    }
}
