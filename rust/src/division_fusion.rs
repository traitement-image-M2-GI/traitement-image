//! Division-fusion (chapitre « Segmentation ») : segmentation par quadtree.
//!
//! On divise récursivement le rectangle courant en quatre jusqu'à ce qu'il soit homogène
//! (écart max-min <= ecart_max) ou trop petit (taille_min). Puis on fusionne les régions
//! voisines dont les moyennes sont proches, pour éviter les découpages inutiles.
//! Chaque région finale est remplacée par sa moyenne.

use std::collections::{HashSet, VecDeque};

use crate::gris;
use crate::pnm::{self, Image};
use crate::{verifier_arguments, Resultat};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rectangle {
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
}

impl Rectangle {
    fn largeur(self) -> usize {
        self.x1 - self.x0
    }
    fn hauteur(self) -> usize {
        self.y1 - self.y0
    }
}

#[derive(Clone, Copy)]
struct Feuille {
    rect: Rectangle,
    moyenne: f64,
}

fn statistiques(image: &Image, rect: Rectangle) -> (u8, u8, f64) {
    let (mut min, mut max, mut somme, mut total) = (255u8, 0u8, 0u64, 0u64);
    for y in rect.y0..rect.y1 {
        for x in rect.x0..rect.x1 {
            let v = image.valeur(x, y, 0);
            min = min.min(v);
            max = max.max(v);
            somme += v as u64;
            total += 1;
        }
    }
    (min, max, somme as f64 / total as f64)
}

fn diviser(image: &Image, rect: Rectangle, ecart_max: u8, taille_min: usize, feuilles: &mut Vec<Feuille>) {
    let (min, max, moyenne) = statistiques(image, rect);
    if max - min <= ecart_max || (rect.largeur() <= taille_min && rect.hauteur() <= taille_min) {
        feuilles.push(Feuille { rect, moyenne });
        return;
    }
    let mx = if rect.largeur() > 1 { rect.x0 + rect.largeur() / 2 } else { rect.x0 };
    let my = if rect.hauteur() > 1 { rect.y0 + rect.hauteur() / 2 } else { rect.y0 };
    let quarts = [
        Rectangle { x0: rect.x0, y0: rect.y0, x1: mx, y1: my },
        Rectangle { x0: mx, y0: rect.y0, x1: rect.x1, y1: my },
        Rectangle { x0: rect.x0, y0: my, x1: mx, y1: rect.y1 },
        Rectangle { x0: mx, y0: my, x1: rect.x1, y1: rect.y1 },
    ];
    for q in quarts {
        if q.largeur() > 0 && q.hauteur() > 0 {
            diviser(image, q, ecart_max, taille_min, feuilles);
        }
    }
}

fn racine(parent: &mut [usize], mut region: usize) -> usize {
    while parent[region] != region {
        parent[region] = parent[parent[region]];
        region = parent[region];
    }
    region
}

fn fusionner(feuilles: &[Feuille], largeur: usize, hauteur: usize, ecart_max: u8) -> Vec<u8> {
    let mut proprietaire = vec![0; largeur * hauteur];
    for (id, feuille) in feuilles.iter().enumerate() {
        for y in feuille.rect.y0..feuille.rect.y1 {
            for x in feuille.rect.x0..feuille.rect.x1 {
                proprietaire[y * largeur + x] = id;
            }
        }
    }

    let mut voisins = vec![HashSet::new(); feuilles.len()];
    let mut aretes = HashSet::new();
    for y in 0..hauteur {
        for x in 0..largeur {
            let id = proprietaire[y * largeur + x];
            for voisin in [
                (x + 1 < largeur).then(|| proprietaire[y * largeur + x + 1]),
                (y + 1 < hauteur).then(|| proprietaire[(y + 1) * largeur + x]),
            ]
            .into_iter()
            .flatten()
            {
                if id != voisin {
                    aretes.insert((id.min(voisin), id.max(voisin)));
                }
            }
        }
    }
    for &(a, b) in &aretes {
        voisins[a].insert(b);
        voisins[b].insert(a);
    }

    let mut parent: Vec<usize> = (0..feuilles.len()).collect();
    let mut moyennes: Vec<f64> = feuilles.iter().map(|f| f.moyenne).collect();
    let mut aires: Vec<usize> = feuilles
        .iter()
        .map(|f| f.rect.largeur() * f.rect.hauteur())
        .collect();
    let mut file: VecDeque<(usize, usize)> = {
        let mut paires: Vec<_> = aretes.into_iter().collect();
        paires.sort_unstable();
        paires.into()
    };
    let tol = ecart_max as f64 / 2.0;

    while let Some((a, b)) = file.pop_front() {
        let (a, b) = (racine(&mut parent, a), racine(&mut parent, b));
        if a == b || (moyennes[a] - moyennes[b]).abs() > tol {
            continue;
        }

        let (garder, retirer) = if aires[a] >= aires[b] { (a, b) } else { (b, a) };
        let aire = aires[garder] + aires[retirer];
        moyennes[garder] = (moyennes[garder] * aires[garder] as f64
            + moyennes[retirer] * aires[retirer] as f64)
            / aire as f64;
        aires[garder] = aire;

        let mut nouveaux_voisins: Vec<_> = voisins[garder]
            .iter()
            .chain(voisins[retirer].iter())
            .copied()
            .map(|id| racine(&mut parent, id))
            .filter(|&id| id != garder && id != retirer)
            .collect();
        nouveaux_voisins.sort_unstable();
        nouveaux_voisins.dedup();

        for &voisin in &nouveaux_voisins {
            voisins[voisin].remove(&garder);
            voisins[voisin].remove(&retirer);
            voisins[voisin].insert(garder);
        }
        voisins[garder] = nouveaux_voisins.iter().copied().collect();
        voisins[retirer].clear();
        parent[retirer] = garder;

        for voisin in nouveaux_voisins {
            file.push_back((garder, voisin));
        }
    }

    proprietaire
        .into_iter()
        .map(|id| moyennes[racine(&mut parent, id)].round().clamp(0.0, 255.0) as u8)
        .collect()
}

/// Remplace chaque région par sa moyenne (ramenée dans [0, 255]).
pub fn appliquer(image: &Image, ecart_max: u8, taille_min: usize) -> Image {
    let gris = gris::vers_gris(image);
    let entier = Rectangle { x0: 0, y0: 0, x1: gris.largeur, y1: gris.hauteur };
    let mut feuilles = Vec::new();
    diviser(&gris, entier, ecart_max, taille_min.max(1), &mut feuilles);

    Image {
        largeur: gris.largeur,
        hauteur: gris.hauteur,
        canaux: 1,
        pixels: fusionner(&feuilles, gris.largeur, gris.hauteur, ecart_max),
    }
}

/// `traitements division-fusion <entrée> <sortie.pnm> <ecart_max> <taille_min>`
pub fn executer(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée", "sortie.pnm", "ecart_max", "taille_min"])?;
    let image = pnm::lire(&args[0])?;
    let ecart: u8 = args[2].parse().map_err(|_| format!("ecart_max invalide : « {} »", args[2]))?;
    let taille: usize = args[3].parse().map_err(|_| format!("taille_min invalide : « {} »", args[3]))?;
    if taille == 0 {
        return Err("taille_min doit etre au moins 1".into());
    }
    pnm::ecrire(&args[1], &appliquer(&image, ecart, taille))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image() -> Image {
        Image::depuis_pixels(2, 2, 1, vec![0, 0, 255, 255]).unwrap()
    }

    #[test]
    fn exemple_fait_a_la_main() {
        assert_eq!(appliquer(&image(), 10, 1).pixels, vec![0, 0, 255, 255]);
    }

    #[test]
    fn sans_division_la_moyenne_est_arondie() {
        assert_eq!(appliquer(&image(), 255, 1).pixels, vec![128, 128, 128, 128]);
        assert_eq!(appliquer(&image(), 10, 2).pixels, vec![128, 128, 128, 128]);
    }

    #[test]
    fn image_deja_homogene() {
        let image = Image::depuis_pixels(2, 2, 1, vec![30, 30, 30, 30]).unwrap();
        assert_eq!(appliquer(&image, 5, 1).pixels, vec![30, 30, 30, 30]);
    }

    #[test]
    fn fusionne_des_regions_qui_ne_partagent_qu_une_partie_du_bord() {
        let feuilles = [
            Feuille {
                rect: Rectangle {
                    x0: 0,
                    y0: 0,
                    x1: 2,
                    y1: 2,
                },
                moyenne: 100.0,
            },
            Feuille {
                rect: Rectangle {
                    x0: 0,
                    y0: 2,
                    x1: 1,
                    y1: 4,
                },
                moyenne: 104.0,
            },
            Feuille {
                rect: Rectangle {
                    x0: 1,
                    y0: 2,
                    x1: 2,
                    y1: 4,
                },
                moyenne: 255.0,
            },
        ];

        assert_eq!(
            fusionner(&feuilles, 2, 4, 20),
            vec![101, 101, 101, 101, 101, 255, 101, 255]
        );
    }
}
