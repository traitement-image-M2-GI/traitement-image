//! Croissance de régions à partir de germes (chapitre « Segmentation »).
//!
//! Chaque germe démarre une région. On ajoute à cette région les pixels voisins non encore
//! visités dont la valeur diffère de celle du germe d'au plus `tolérance`. La connectivité
//! est choisie parmi 4 ou 8 voisins. La sortie est une image d'étiquettes étalées sur 0..255.

use crate::gris;
use crate::pnm::{self, Image};
use crate::{verifier_arguments, Resultat};
use std::collections::VecDeque;
use std::fs;

fn lire_germes(texte: &str, largeur: usize, hauteur: usize) -> Result<Vec<(usize, usize)>, String> {
    let nombres = texte
        .split(',')
        .map(str::trim)
        .filter(|morceau| !morceau.is_empty())
        .map(|morceau| {
            morceau
                .parse::<usize>()
                .map_err(|_| format!("germe invalide : « {morceau} » (entier attendu)"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if nombres.len() < 2 || nombres.len() % 2 != 0 {
        return Err("les germes doivent etre donnes par paires x,y".into());
    }
    let germes: Vec<_> = nombres.chunks(2).map(|p| (p[0], p[1])).collect();
    for &(x, y) in &germes {
        if x >= largeur || y >= hauteur {
            return Err(format!("germe ({x},{y}) hors de l'image {largeur}x{hauteur}").into());
        }
    }
    Ok(germes)
}

/// Renvoie (image des étiquettes, taille de chaque région).
pub fn croissance(image: &Image, germes: &[(usize, usize)], tol: u8, connexite: u8) -> (Image, Vec<usize>) {
    let gris = gris::vers_gris(image);
    let mut etiquettes: Vec<Option<usize>> = vec![None; gris.nb_pixels()];
    let mut tailles = Vec::new();
    let voisins: &[(i64, i64)] = if connexite == 8 {
        &[(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)]
    } else {
        &[(0, -1), (-1, 0), (1, 0), (0, 1)]
    };

    for &(gx, gy) in germes {
        let depart = gy * gris.largeur + gx;
        if etiquettes[depart].is_some() {
            continue;
        }
        let id = tailles.len();
        let reference = gris.valeur(gx, gy, 0);
        let mut file = VecDeque::new();
        etiquettes[depart] = Some(id);
        file.push_back((gx, gy));
        let mut taille = 0;

        while let Some((x, y)) = file.pop_front() {
            taille += 1;
            for &(dx, dy) in voisins {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx < 0 || ny < 0 {
                    continue;
                }
                let (nx, ny) = (nx as usize, ny as usize);
                if nx >= gris.largeur || ny >= gris.hauteur {
                    continue;
                }
                let index = ny * gris.largeur + nx;
                if etiquettes[index].is_some() {
                    continue;
                }
                if gris.valeur(nx, ny, 0).abs_diff(reference) <= tol {
                    etiquettes[index] = Some(id);
                    file.push_back((nx, ny));
                }
            }
        }
        tailles.push(taille);
    }

    let nb = tailles.len().max(1);
    let pixels = etiquettes
        .iter()
        .map(|e| match e {
            Some(id) => ((id + 1) * 255 / nb) as u8,
            None => 0,
        })
        .collect();
    (Image::depuis_pixels(gris.largeur, gris.hauteur, 1, pixels).unwrap(), tailles)
}

fn vers_json(tailles: &[usize]) -> String {
    let mut valeurs = vec![format!("\"regions\": {}", tailles.len())];
    for (i, taille) in tailles.iter().enumerate() {
        valeurs.push(format!("\"region_{i}\": {taille}"));
    }
    format!("{{\"valeurs\": {{{}}}}}\n", valeurs.join(", "))
}

/// `traitements croissance <entrée> <sortie.pnm> <sortie.json> <germes> <tolérance> <connexité>`
pub fn executer(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée", "sortie.pnm", "sortie.json", "germes", "tolérance", "connexité"])?;
    let image = pnm::lire(&args[0])?;
    let germes = lire_germes(&args[3], image.largeur, image.hauteur)?;
    let tol: u8 = args[4].parse().map_err(|_| format!("tolérance invalide : « {} »", args[4]))?;
    let connexite: u8 = args[5].parse().map_err(|_| format!("connexité invalide : « {} »", args[5]))?;
    if connexite != 4 && connexite != 8 {
        return Err("la connexité doit valoir 4 ou 8".into());
    }
    let (sortie, tailles) = croissance(&image, &germes, tol, connexite);
    pnm::ecrire(&args[1], &sortie)?;
    fs::write(&args[2], vers_json(&tailles))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image() -> Image {
        Image::depuis_pixels(3, 3, 1, vec![0, 0, 0, 0, 50, 0, 0, 0, 0]).unwrap()
    }

    #[test]
    fn exemple_fait_a_la_main() {
        let (sortie, tailles) = croissance(&image(), &[(0, 0), (1, 1)], 10, 8);
        assert_eq!(tailles, vec![8, 1]);
        assert_eq!(sortie.pixels, vec![127, 127, 127, 127, 255, 127, 127, 127, 127]);
    }

    #[test]
    fn lecture_des_germes() {
        assert_eq!(lire_germes("1,2,3,4", 10, 10).unwrap(), vec![(1, 2), (3, 4)]);
        assert!(lire_germes("1", 10, 10).is_err());
        assert!(lire_germes("1,2,3", 10, 10).is_err());
        assert!(lire_germes("12,2", 10, 10).is_err());
    }

    #[test]
    fn germe_en_double_ne_cree_qu_une_region() {
        let (_, tailles) = croissance(&image(), &[(0, 0), (0, 0)], 10, 4);
        assert_eq!(tailles, vec![8]);
    }
}
