//! Division-fusion (chapitre « Segmentation ») : segmentation par quadtree.
//!
//! On divise récursivement le rectangle courant en quatre jusqu'à ce qu'il soit homogène
//! (écart max-min <= ecart_max) ou trop petit (taille_min). Puis on fusionne les régions
//! voisines dont les moyennes sont proches, pour éviter les découpages inutiles.
//! Chaque région finale est remplacée par sa moyenne.

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

fn adjacents(a: Rectangle, b: Rectangle) -> bool {
    (a.x0 == b.x0 && a.x1 == b.x1 && (a.y1 == b.y0 || b.y1 == a.y0))
        || (a.y0 == b.y0 && a.y1 == b.y1 && (a.x1 == b.x0 || b.x1 == a.x0))
}

fn fusionner(feuilles: &mut Vec<Feuille>, ecart_max: u8) {
    let tol = ecart_max as f64 / 2.0;
    loop {
        let mut paire = None;
        'recherche: for i in 0..feuilles.len() {
            for j in i + 1..feuilles.len() {
                let (a, b) = (feuilles[i], feuilles[j]);
                if adjacents(a.rect, b.rect) && (a.moyenne - b.moyenne).abs() <= tol {
                    paire = Some((i, j));
                    break 'recherche;
                }
            }
        }
        let Some((i, j)) = paire else { break };
        let a = feuilles.remove(i);
        let b = feuilles.remove(j - 1);
        let na = (a.rect.largeur() * a.rect.hauteur()) as f64;
        let nb = (b.rect.largeur() * b.rect.hauteur()) as f64;
        let rect = Rectangle {
            x0: a.rect.x0.min(b.rect.x0),
            y0: a.rect.y0.min(b.rect.y0),
            x1: a.rect.x1.max(b.rect.x1),
            y1: a.rect.y1.max(b.rect.y1),
        };
        feuilles.push(Feuille { rect, moyenne: (a.moyenne * na + b.moyenne * nb) / (na + nb) });
    }
}

/// Remplace chaque région par sa moyenne (ramenée dans [0, 255]).
pub fn appliquer(image: &Image, ecart_max: u8, taille_min: usize) -> Image {
    let gris = gris::vers_gris(image);
    let entier = Rectangle { x0: 0, y0: 0, x1: gris.largeur, y1: gris.hauteur };
    let mut feuilles = Vec::new();
    diviser(&gris, entier, ecart_max, taille_min.max(1), &mut feuilles);
    fusionner(&mut feuilles, ecart_max);

    let mut sortie = Image::nouvelle(gris.largeur, gris.hauteur, 1);
    for feuille in feuilles {
        let valeur = feuille.moyenne.round().clamp(0.0, 255.0) as u8;
        for y in feuille.rect.y0..feuille.rect.y1 {
            for x in feuille.rect.x0..feuille.rect.x1 {
                sortie.modifier(x, y, 0, valeur);
            }
        }
    }
    sortie
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
}
