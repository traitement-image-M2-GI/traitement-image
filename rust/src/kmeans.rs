//! K-moyennes (chapitre « Segmentation ») : regroupe les pixels en k classes.
//!
//! Les pixel sont soit des nombres (image grise), soit des triplets RVB (image couleur).
//! Initialisation déterministe : k pixels également répartis sur les valeurs triées, pour que
//! deux exécutions donnent toujours le même résultat. Itérations de Lloyd jusqu'à stabilisation.

use crate::pnm::{self, Image};
use crate::{verifier_arguments, Resultat};
use std::fs;

type Pixel = Vec<f64>;

fn distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).powi(2)).sum()
}

fn init_centroides(pixels: &[Pixel], k: usize) -> Vec<Pixel> {
    let mut ordonnes = pixels.to_vec();
    ordonnes.sort_by(|a, b| {
        let sa: f64 = a.iter().sum();
        let sb: f64 = b.iter().sum();
        sa.partial_cmp(&sb).unwrap()
    });
    if k <= 1 {
        return vec![ordonnes[0].clone()];
    }
    (0..k).map(|i| ordonnes[i * (ordonnes.len() - 1) / (k - 1)].clone()).collect()
}

/// Renvoie (image quantifiée, centroïdes, effectifs, nombre d'itérations).
pub fn kmeans(image: &Image, k: usize, iterations: usize) -> (Image, Vec<Pixel>, Vec<usize>, usize) {
    let dim = image.canaux;
    let pixels: Vec<Pixel> = image.pixels.chunks_exact(dim).map(|p| p.iter().map(|&v| v as f64).collect()).collect();
    let k = k.max(1).min(pixels.len());
    let mut centroides = init_centroides(&pixels, k);
    let mut classes: Vec<usize> = vec![usize::MAX; pixels.len()];
    let mut effectifs = vec![0usize; k];
    let mut faites = 0;

    for _ in 0..iterations {
        let mut change = false;
        for (i, p) in pixels.iter().enumerate() {
            let meilleur = (0..k)
                .min_by(|&a, &b| distance(p, &centroides[a]).partial_cmp(&distance(p, &centroides[b])).unwrap())
                .unwrap();
            if classes[i] != meilleur {
                change = true;
            }
            classes[i] = meilleur;
        }
        faites += 1;

        let mut sommes = vec![vec![0.0; dim]; k];
        effectifs.fill(0);
        for (p, &classe) in pixels.iter().zip(&classes) {
            effectifs[classe] += 1;
            for (d, &v) in p.iter().enumerate() {
                sommes[classe][d] += v;
            }
        }
        for c in 0..k {
            if effectifs[c] > 0 {
                for d in 0..dim {
                    centroides[c][d] = (sommes[c][d] / effectifs[c] as f64).round();
                }
            }
        }
        if !change {
            break;
        }
    }

    let resultat: Vec<u8> = pixels
        .iter()
        .zip(&classes)
        .flat_map(|(_, &classe)| centroides[classe].iter().map(|&v| v as u8))
        .collect();
    let image_sortie = Image::depuis_pixels(image.largeur, image.hauteur, dim, resultat).unwrap();
    (image_sortie, centroides, effectifs, faites)
}

fn vers_json(k: usize, faites: usize, centroides: &[Pixel], effectifs: &[usize]) -> String {
    let mut valeurs = vec![format!("\"k\": {k}"), format!("\"iterations\": {faites}")];
    for (i, c) in centroides.iter().enumerate() {
        let texte = if c.len() == 1 {
            format!("{}", c[0] as u8)
        } else {
            format!("({}, {}, {})", c[0] as u8, c[1] as u8, c[2] as u8)
        };
        valeurs.push(format!("\"centroide_{i}\": \"{texte}\""));
        valeurs.push(format!("\"pixels_{i}\": {}", effectifs[i]));
    }
    format!("{{\"valeurs\": {{{}}}}}\n", valeurs.join(", "))
}

/// `traitements kmeans <entrée> <sortie.pnm> <sortie.json> <k> <iterations>`
pub fn executer(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée", "sortie.pnm", "sortie.json", "k", "iterations"])?;
    let image = pnm::lire(&args[0])?;
    let k: usize = args[3].parse().map_err(|_| format!("k invalide : « {} »", args[3]))?;
    let iterations: usize = args[4].parse().map_err(|_| format!("nombre d'itérations invalide : « {} »", args[4]))?;
    if k < 2 {
        return Err("k doit être au moins 2".into());
    }
    if iterations == 0 {
        return Err("au moins une itération est attendue".into());
    }
    let (sortie, centroides, effectifs, faites) = kmeans(&image, k, iterations);
    pnm::ecrire(&args[1], &sortie)?;
    fs::write(&args[2], vers_json(k, faites, &centroides, &effectifs))?;
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
        let image = gris(vec![0, 0, 10, 10, 100, 100]);
        let (sortie, centroides, _, faites) = kmeans(&image, 2, 30);
        assert_eq!(sortie.pixels, vec![5, 5, 5, 5, 100, 100]);
        assert_eq!(centroides.iter().map(|c| c[0] as u8).collect::<Vec<_>>(), vec![5, 100]);
        assert_eq!(faites, 2);
    }

    #[test]
    fn couleurs_primaires_restent_separees() {
        let image = Image::depuis_pixels(2, 1, 3, vec![0, 0, 0, 255, 255, 255]).unwrap();
        let (sortie, _, effectifs, _) = kmeans(&image, 2, 20);
        assert_eq!(sortie.pixels, vec![0, 0, 0, 255, 255, 255]);
        assert_eq!(effectifs, vec![1, 1]);
    }

    #[test]
    fn k_superieur_au_nombre_de_pixels_est_ramene() {
        let image = gris(vec![42]);
        let (sortie, centroides, _, _) = kmeans(&image, 5, 10);
        assert_eq!(sortie.pixels, vec![42]);
        assert_eq!(centroides.len(), 1);
    }
}
