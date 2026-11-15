//! Histogramme d'une image (cours 02 « Traitement de base », diapos 2 à 5).
//!
//! H(k) = nombre de pixels de l'image ayant la valeur k, pour k de 0 à 255.
//! Pour une image couleur : un histogramme par canal (R, G, B).
//!
//! Sortie : un fichier JSON (format décrit dans DEVELOPPEMENT.md) avec
//!   - "courbes" : un histogramme par canal (256 valeurs), tracé en barres par l'interface
//!   - "valeurs" : luminance (moyenne), contraste (écart-type), dynamique (min, max)

use crate::pnm::{self, Image};
use crate::{verifier_arguments, Resultat};
use std::fs;

/// Calcule l'histogramme du canal `canal` (0 en gris ; 0, 1, 2 = R, G, B en couleur).
pub fn calculer(image: &Image, canal: usize) -> [u64; 256] {
    let mut h = [0u64; 256];
    // Les valeurs du canal `canal` sont aux indices canal, canal + canaux, canal + 2 × canaux…
    for &v in image.pixels.iter().skip(canal).step_by(image.canaux) {
        h[v as usize] += 1;
    }
    h
}

/// Statistiques d'un canal, calculées à partir de son histogramme.
pub struct Statistiques {
    pub moyenne: f64,    // luminance (diapo 4)
    pub ecart_type: f64, // contraste (diapo 5)
    pub min: u8,         // dynamique [min, max] (diapo 3)
    pub max: u8,
}

pub fn statistiques(h: &[u64; 256]) -> Statistiques {
    let n: u64 = h.iter().sum();
    let n_f = n as f64;
    let moyenne = h.iter().enumerate().map(|(k, &c)| k as f64 * c as f64).sum::<f64>() / n_f;
    let variance = h.iter().enumerate().map(|(k, &c)| c as f64 * (k as f64 - moyenne).powi(2)).sum::<f64>() / n_f;
    // premier et dernier niveau présent (l'image a au moins un pixel)
    let min = h.iter().position(|&c| c > 0).unwrap_or(0) as u8;
    let max = h.iter().rposition(|&c| c > 0).unwrap_or(0) as u8;
    Statistiques { moyenne, ecart_type: variance.sqrt(), min, max }
}

/// `traitements histogramme <entrée> <sortie.json>`
pub fn executer(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée", "sortie.json"])?;
    let image = pnm::lire(&args[0])?;
    fs::write(&args[1], vers_json(&image))?;
    Ok(())
}

/// Construit le JSON de sortie (écrit à la main : pas de bibliothèque externe).
fn vers_json(image: &Image) -> String {
    let canaux: &[(&str, &str, &str)] = if image.est_couleur() {
        &[("Rouge", "R", "#e34948"), ("Vert", "G", "#008300"), ("Bleu", "B", "#2a78d6")]
    } else {
        &[("Niveaux de gris", "", "")]
    };

    let mut courbes = Vec::new();
    let mut valeurs = Vec::new();
    for (c, (nom, court, couleur)) in canaux.iter().enumerate() {
        let h = calculer(image, c);
        let liste = h.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",");
        let couleur = if couleur.is_empty() { String::new() } else { format!(", \"couleur\": \"{couleur}\"") };
        courbes.push(format!("{{\"nom\": \"{nom}\", \"style\": \"barres\"{couleur}, \"valeurs\": [{liste}]}}"));

        let s = statistiques(&h);
        let suffixe = if court.is_empty() { String::new() } else { format!(" ({court})") };
        valeurs.push(format!("\"moyenne{suffixe}\": {:.2}", s.moyenne));
        valeurs.push(format!("\"écart-type{suffixe}\": {:.2}", s.ecart_type));
        valeurs.push(format!("\"min{suffixe}\": {}", s.min));
        valeurs.push(format!("\"max{suffixe}\": {}", s.max));
    }
    valeurs.push(format!("\"pixels\": {}", image.nb_pixels()));

    format!("{{\n\"courbes\": [{}],\n\"valeurs\": {{{}}}\n}}\n", courbes.join(",\n"), valeurs.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compte_chaque_niveau() {
        let image = Image::depuis_pixels(4, 2, 1, vec![100, 100, 100, 101, 101, 102, 103, 103]).unwrap();
        let h = calculer(&image, 0);
        assert_eq!((h[100], h[101], h[102], h[103]), (3, 2, 1, 2));
        assert_eq!(h.iter().sum::<u64>(), 8); // chaque pixel compté une fois
    }

    #[test]
    fn un_histogramme_par_canal_en_couleur() {
        // 2 pixels : (255, 0, 10) et (255, 0, 20)
        let image = Image::depuis_pixels(2, 1, 3, vec![255, 0, 10, 255, 0, 20]).unwrap();
        assert_eq!(calculer(&image, 0)[255], 2); // R
        assert_eq!(calculer(&image, 1)[0], 2); // G
        assert_eq!((calculer(&image, 2)[10], calculer(&image, 2)[20]), (1, 1)); // B
    }

    #[test]
    fn statistiques_simples() {
        let image = Image::depuis_pixels(4, 1, 1, vec![0, 0, 255, 255]).unwrap();
        let s = statistiques(&calculer(&image, 0));
        assert_eq!((s.min, s.max), (0, 255));
        assert!((s.moyenne - 127.5).abs() < 1e-9);
        assert!((s.ecart_type - 127.5).abs() < 1e-9);
    }

    #[test]
    fn json_contient_courbes_et_valeurs() {
        let image = Image::depuis_pixels(2, 1, 1, vec![10, 20]).unwrap();
        let json = vers_json(&image);
        assert!(json.contains("\"courbes\"") && json.contains("\"valeurs\""));
        assert!(json.contains("\"moyenne\": 15.00"));
    }
}
