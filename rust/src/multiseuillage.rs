//! Multi-seuillage (chapitre « Segmentation ») : n seuils donnent n+1 classes.
//!
//! La classe d'un pixel est le nombre de seuils inférieurs ou égaux à sa valeur.
//! Les classes sont étalées sur 0..255 pour rester lisibles : avec 3 seuils,
//! les 4 classes valent 0, 85, 170 et 255.

use crate::gris;
use crate::pnm::{self, Image};
use crate::{verifier_arguments, Resultat};

/// Applique les seuils (triés et dédupliqués) et renvoie l'image à n+1 classes.
pub fn appliquer(image: &Image, seuils: &[u8]) -> Image {
    let gris = gris::vers_gris(image);
    let mut seuils = seuils.to_vec();
    seuils.sort_unstable();
    seuils.dedup();
    let diviseur = seuils.len().max(1) as u16;
    let pixels = gris
        .pixels
        .iter()
        .map(|&v| {
            let classe = seuils.iter().take_while(|&&s| s <= v).count() as u16;
            ((classe * 255) / diviseur) as u8
        })
        .collect();
    Image::depuis_pixels(image.largeur, image.hauteur, 1, pixels).expect("même nombre de pixels que l'entrée")
}

fn lire_seuils(texte: &str) -> Result<Vec<u8>, String> {
    let seuils: Vec<u8> = texte
        .split(',')
        .map(str::trim)
        .filter(|morceau| !morceau.is_empty())
        .map(|morceau| {
            morceau
                .parse::<u8>()
                .map_err(|_| format!("seuil invalide : « {morceau} » (entier entre 0 et 255 attendu)"))
        })
        .collect::<Result<_, _>>()?;
    if seuils.is_empty() {
        return Err("au moins un seuil est attendu".into());
    }
    Ok(seuils)
}

/// `traitements multiseuillage <entrée> <sortie.pnm> <seuil1,seuil2,...>`
pub fn executer(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée", "sortie.pnm", "seuils"])?;
    let image = pnm::lire(&args[0])?;
    let seuils = lire_seuils(&args[2])?;
    pnm::ecrire(&args[1], &appliquer(&image, &seuils))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gris(pixels: Vec<u8>) -> Image {
        Image::depuis_pixels(pixels.len(), 1, 1, pixels).unwrap()
    }

    #[test]
    fn exemple_fait_a_la_main() {
        // 3 seuils -> 4 classes : 0, 85, 170, 255
        let image = gris(vec![0, 50, 100, 150, 200, 255]);
        let resultat = appliquer(&image, &[64, 128, 192]);
        assert_eq!(resultat.pixels, vec![0, 0, 85, 170, 255, 255]);
    }

    #[test]
    fn seuils_tries_et_dupliques() {
        let image = gris(vec![0, 50, 100, 150, 200, 255]);
        assert_eq!(appliquer(&image, &[192, 64, 128, 64]), appliquer(&image, &[64, 128, 192]));
    }

    #[test]
    fn lecture_de_la_liste() {
        assert_eq!(lire_seuils("64, 128, 192").unwrap(), vec![64, 128, 192]);
        assert!(lire_seuils("").is_err());
        assert!(lire_seuils("300").is_err());
    }
}
