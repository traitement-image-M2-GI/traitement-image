//! Détection de contours (lot « Contours », Cinthia).
//!
//! Idée : un contour est un endroit où la luminosité change brusquement entre deux pixels
//! voisins. Pour le trouver, on mesure ce changement (le « gradient ») avec de petits noyaux
//! de convolution (Roberts, Prewitt, Sobel), puis on nettoie le résultat.
//!
//! Chaîne classique (l'interface permet d'enchaîner les traitements) :
//!
//! ```text
//!   image ──► gradient (norme) ──► amincissement ──► hystérésis ──► fermeture des contours
//!                   └─► seuillage simple                                  (contours finaux)
//!                   └─► Hough (détection de droites)
//! ```
//!
//! Choix communs à tous les traitements de ce fichier :
//!   - Les calculs se font en nombres décimaux (`f64`) dans une `Carte`, car un gradient peut
//!     être NÉGATIF (passage du clair au sombre). On ne ramène dans [0, 255] qu'à la toute fin.
//!   - Bords de l'image : un pixel qui sort de l'image est remplacé par le pixel du bord le plus
//!     proche (« bords répliqués »). Ainsi il n'y a pas de faux contour sur le cadre de l'image.
//!   - Image couleur : convertie en niveaux de gris (0,299 R + 0,587 G + 0,114 B).
//!   - L'axe y est orienté vers le BAS (origine en haut à gauche, comme dans tout le projet).
//!
//! Sous-commandes :
//!   derivee, gradient, laplacien, hough, amincissement, hysteresis, fermeture-contours.

use crate::pnm::{self, Image};
use crate::{verifier_arguments, Resultat};
use std::f64::consts::PI;
use std::fs;

// ===========================================================================
// La « Carte » : une image de nombres décimaux (peut contenir des valeurs négatives)
// ===========================================================================

struct Carte {
    largeur: usize,
    hauteur: usize,
    valeurs: Vec<f64>,
}

impl Carte {
    fn nouvelle(largeur: usize, hauteur: usize) -> Carte {
        Carte { largeur, hauteur, valeurs: vec![0.0; largeur * hauteur] }
    }

    /// Convertit une image (gris ou couleur) en carte de niveaux de gris.
    fn depuis_image(image: &Image) -> Carte {
        let mut carte = Carte::nouvelle(image.largeur, image.hauteur);
        for y in 0..image.hauteur {
            for x in 0..image.largeur {
                let v = if image.canaux == 1 {
                    image.valeur(x, y, 0) as f64
                } else {
                    0.299 * image.valeur(x, y, 0) as f64
                        + 0.587 * image.valeur(x, y, 1) as f64
                        + 0.114 * image.valeur(x, y, 2) as f64
                };
                carte.valeurs[y * image.largeur + x] = v;
            }
        }
        carte
    }

    /// Valeur en (x, y). Hors de l'image, on prend le pixel du bord le plus proche.
    fn valeur(&self, x: i64, y: i64) -> f64 {
        let xx = x.clamp(0, self.largeur as i64 - 1) as usize;
        let yy = y.clamp(0, self.hauteur as i64 - 1) as usize;
        self.valeurs[yy * self.largeur + xx]
    }

    fn fixer(&mut self, x: usize, y: usize, v: f64) {
        self.valeurs[y * self.largeur + x] = v;
    }

    /// Plus grande valeur absolue de la carte.
    fn max_abs(&self) -> f64 {
        self.valeurs.iter().fold(0.0, |m, v| m.max(v.abs()))
    }

    /// Image en niveaux de gris : |valeur| ramenée dans [0, 255] (le plus grand devient 255).
    fn vers_image_abs(&self) -> Image {
        let max = self.max_abs();
        let pixels = self
            .valeurs
            .iter()
            .map(|v| if max > 0.0 { (v.abs() / max * 255.0).round() as u8 } else { 0 })
            .collect();
        Image { largeur: self.largeur, hauteur: self.hauteur, canaux: 1, pixels }
    }
}

/// Image binaire (0 ou 255) à partir d'un tableau de booléens.
fn image_binaire(largeur: usize, hauteur: usize, actifs: &[bool]) -> Image {
    let pixels = actifs.iter().map(|&a| if a { 255 } else { 0 }).collect();
    Image { largeur, hauteur, canaux: 1, pixels }
}

// ===========================================================================
// Lecture des paramètres
// ===========================================================================

fn lire_entier(texte: &str, nom: &str, min: i64, max: i64) -> Result<i64, String> {
    let n: i64 = texte.parse().map_err(|_| format!("{nom} invalide : « {texte} » (entier attendu)"))?;
    if n < min || n > max {
        return Err(format!("{nom} doit être entre {min} et {max} (reçu {n})"));
    }
    Ok(n)
}

// ===========================================================================
// 1. Dérivée discrète en X ou en Y
// ===========================================================================

/// Dérivée discrète : différence entre un pixel et son voisin de droite (X) ou du dessous (Y).
///   dérivée en X : f(x+1, y) − f(x, y)        dérivée en Y : f(x, y+1) − f(x, y)
fn derivee(g: &Carte, axe: &str) -> Result<Carte, String> {
    let (dx, dy) = match axe {
        "x" => (1, 0),
        "y" => (0, 1),
        autre => return Err(format!("axe inconnu : « {autre} » (x ou y)")),
    };
    let mut sortie = Carte::nouvelle(g.largeur, g.hauteur);
    for y in 0..g.hauteur {
        for x in 0..g.largeur {
            let (xi, yi) = (x as i64, y as i64);
            sortie.fixer(x, y, g.valeur(xi + dx, yi + dy) - g.valeur(xi, yi));
        }
    }
    Ok(sortie)
}

/// `traitements derivee <entrée> <sortie> <x|y>`
pub fn executer_derivee(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée", "sortie", "axe (x|y)"])?;
    let g = Carte::depuis_image(&pnm::lire(&args[0])?);
    let d = derivee(&g, &args[2])?;
    // La dérivée peut être négative : on affiche sa valeur absolue
    pnm::ecrire(&args[1], &d.vers_image_abs())?;
    Ok(())
}

// ===========================================================================
// 2. Gradient : Roberts, Prewitt, Sobel (norme, direction, seuillage)
// ===========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operateur {
    Roberts,
    Prewitt,
    Sobel,
}

impl Operateur {
    fn depuis_texte(texte: &str) -> Result<Operateur, String> {
        match texte {
            "roberts" => Ok(Operateur::Roberts),
            "prewitt" => Ok(Operateur::Prewitt),
            "sobel" => Ok(Operateur::Sobel),
            autre => Err(format!("opérateur inconnu : « {autre} » (roberts, prewitt ou sobel)")),
        }
    }
}

/// Calcule les deux composantes du gradient (gx, gy) en chaque pixel.
///
/// Noyaux utilisés (f(dx, dy) = pixel décalé de dx colonnes et dy lignes) :
///   Roberts (2×2) : gx = f(0,0) − f(1,1)               gy = f(1,0) − f(0,1)
///   Prewitt (3×3) : gx = colonne droite − colonne gauche (poids 1, 1, 1)
///                   gy = ligne du bas − ligne du haut     (poids 1, 1, 1)
///   Sobel   (3×3) : comme Prewitt, mais le pixel central compte double (poids 1, 2, 1)
fn gradient(g: &Carte, op: Operateur) -> (Carte, Carte) {
    let mut gx = Carte::nouvelle(g.largeur, g.hauteur);
    let mut gy = Carte::nouvelle(g.largeur, g.hauteur);
    for y in 0..g.hauteur {
        for x in 0..g.largeur {
            let (xi, yi) = (x as i64, y as i64);
            let f = |dx: i64, dy: i64| g.valeur(xi + dx, yi + dy);
            let (vx, vy) = match op {
                Operateur::Roberts => (f(0, 0) - f(1, 1), f(1, 0) - f(0, 1)),
                Operateur::Prewitt => (
                    (f(1, -1) + f(1, 0) + f(1, 1)) - (f(-1, -1) + f(-1, 0) + f(-1, 1)),
                    (f(-1, 1) + f(0, 1) + f(1, 1)) - (f(-1, -1) + f(0, -1) + f(1, -1)),
                ),
                Operateur::Sobel => (
                    (f(1, -1) + 2.0 * f(1, 0) + f(1, 1)) - (f(-1, -1) + 2.0 * f(-1, 0) + f(-1, 1)),
                    (f(-1, 1) + 2.0 * f(0, 1) + f(1, 1)) - (f(-1, -1) + 2.0 * f(0, -1) + f(1, -1)),
                ),
            };
            gx.fixer(x, y, vx);
            gy.fixer(x, y, vy);
        }
    }
    (gx, gy)
}

/// Norme du gradient : √(gx² + gy²) = « force » du contour (toujours ≥ 0).
fn norme(gx: &Carte, gy: &Carte) -> Carte {
    let mut n = Carte::nouvelle(gx.largeur, gx.hauteur);
    for i in 0..n.valeurs.len() {
        n.valeurs[i] = (gx.valeurs[i] * gx.valeurs[i] + gy.valeurs[i] * gy.valeurs[i]).sqrt();
    }
    n
}

/// Image de sortie du gradient, selon `affichage` : norme, gx, gy, direction ou seuillage.
fn gradient_vers_image(g: &Carte, op: Operateur, affichage: &str, seuil: u8) -> Result<Image, String> {
    let (gx, gy) = gradient(g, op);
    match affichage {
        "norme" => Ok(norme(&gx, &gy).vers_image_abs()),
        "gx" => Ok(gx.vers_image_abs()),
        "gy" => Ok(gy.vers_image_abs()),
        "direction" => {
            // Angle du gradient (perpendiculaire au contour) : de −π à π, ramené à [0, 255].
            // Là où il n'y a pas de gradient, la direction n'a pas de sens : on met 0.
            let n = norme(&gx, &gy);
            let mut pixels = vec![0u8; n.valeurs.len()];
            for i in 0..pixels.len() {
                if n.valeurs[i] > 1e-9 {
                    let angle = gy.valeurs[i].atan2(gx.valeurs[i]);
                    pixels[i] = ((angle + PI) / (2.0 * PI) * 255.0).round() as u8;
                }
            }
            Ok(Image { largeur: g.largeur, hauteur: g.hauteur, canaux: 1, pixels })
        }
        "seuillage" => {
            // On garde (255) les pixels dont la norme, ramenée à [0, 255], atteint le seuil
            let en_gris = norme(&gx, &gy).vers_image_abs();
            let actifs: Vec<bool> = en_gris.pixels.iter().map(|&p| p >= seuil).collect();
            Ok(image_binaire(g.largeur, g.hauteur, &actifs))
        }
        autre => Err(format!("affichage inconnu : « {autre} » (norme, gx, gy, direction ou seuillage)")),
    }
}

/// `traitements gradient <entrée> <sortie> <roberts|prewitt|sobel> <norme|gx|gy|direction|seuillage> <seuil 0..255>`
pub fn executer_gradient(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée", "sortie", "opérateur", "affichage", "seuil"])?;
    let g = Carte::depuis_image(&pnm::lire(&args[0])?);
    let op = Operateur::depuis_texte(&args[2])?;
    let seuil = lire_entier(&args[4], "seuil", 0, 255)? as u8;
    pnm::ecrire(&args[1], &gradient_vers_image(&g, op, &args[3], seuil)?)?;
    Ok(())
}

// ===========================================================================
// 3. Laplacien et passages par zéro
// ===========================================================================

/// Laplacien (dérivée seconde) : somme des voisins moins (4 ou 8) fois le pixel central.
///   4 voisins : haut + bas + gauche + droite − 4·centre
///   8 voisins : les 8 pixels autour − 8·centre
fn laplacien(g: &Carte, voisins: &str) -> Result<Carte, String> {
    let huit = match voisins {
        "4" => false,
        "8" => true,
        autre => return Err(format!("voisinage inconnu : « {autre} » (4 ou 8)")),
    };
    let mut sortie = Carte::nouvelle(g.largeur, g.hauteur);
    for y in 0..g.hauteur {
        for x in 0..g.largeur {
            let (xi, yi) = (x as i64, y as i64);
            let f = |dx: i64, dy: i64| g.valeur(xi + dx, yi + dy);
            let mut somme = f(0, -1) + f(0, 1) + f(-1, 0) + f(1, 0) - 4.0 * f(0, 0);
            if huit {
                somme += f(-1, -1) + f(1, -1) + f(-1, 1) + f(1, 1) - 4.0 * f(0, 0);
            }
            sortie.fixer(x, y, somme);
        }
    }
    Ok(sortie)
}

/// Passages par zéro du laplacien : un contour se trouve là où le laplacien CHANGE DE SIGNE.
/// On compare chaque pixel à son voisin de droite et à celui du dessous ; si les signes sont
/// opposés, on marque celui des deux qui est le plus proche de 0. Pour ignorer le bruit, le saut
/// entre les deux valeurs doit valoir au moins `seuil`/255 du plus grand laplacien de l'image.
fn passages_par_zero(l: &Carte, seuil: u8) -> Image {
    let minimum = l.max_abs() * seuil as f64 / 255.0;
    let mut actifs = vec![false; l.valeurs.len()];
    for y in 0..l.hauteur {
        for x in 0..l.largeur {
            let a = l.valeurs[y * l.largeur + x];
            for (dx, dy) in [(1usize, 0usize), (0, 1)] {
                let (xx, yy) = (x + dx, y + dy);
                if xx >= l.largeur || yy >= l.hauteur {
                    continue;
                }
                let b = l.valeurs[yy * l.largeur + xx];
                if a * b < 0.0 && (a - b).abs() >= minimum {
                    if a.abs() <= b.abs() {
                        actifs[y * l.largeur + x] = true;
                    } else {
                        actifs[yy * l.largeur + xx] = true;
                    }
                }
            }
        }
    }
    image_binaire(l.largeur, l.hauteur, &actifs)
}

/// `traitements laplacien <entrée> <sortie> <4|8> <laplacien|passages-zero> <seuil 0..255>`
pub fn executer_laplacien(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée", "sortie", "voisinage (4|8)", "affichage", "seuil"])?;
    let g = Carte::depuis_image(&pnm::lire(&args[0])?);
    let l = laplacien(&g, &args[2])?;
    let seuil = lire_entier(&args[4], "seuil", 0, 255)? as u8;
    let image = match args[3].as_str() {
        "laplacien" => l.vers_image_abs(),
        "passages-zero" => passages_par_zero(&l, seuil),
        autre => return Err(format!("affichage inconnu : « {autre} » (laplacien ou passages-zero)").into()),
    };
    pnm::ecrire(&args[1], &image)?;
    Ok(())
}

// ===========================================================================
// 4. Transformée de Hough (détection de droites)
// ===========================================================================

/// Nombre d'angles testés : θ = 0°, 1°, …, 179°.
const NB_THETA: usize = 180;

/// Une droite est décrite par (ρ, θ) : ρ = x·cos θ + y·sin θ.
/// ρ est la distance (signée) entre l'origine (coin haut-gauche) et la droite ;
/// θ est l'angle de la perpendiculaire à la droite (θ = 90° : droite horizontale).
struct Hough {
    diagonale: i64,
    nb_rho: usize,
    /// accumulateur : `acc[(ρ + diagonale) * NB_THETA + θ]` = nombre de votes
    acc: Vec<u32>,
}

/// Chaque pixel de contour (valeur ≥ `seuil`) vote pour toutes les droites qui passent par lui.
/// Retourne l'accumulateur et le nombre de pixels de contour.
fn hough(g: &Carte, seuil: f64) -> (Hough, usize) {
    let diagonale = ((g.largeur * g.largeur + g.hauteur * g.hauteur) as f64).sqrt().ceil() as i64;
    let nb_rho = (2 * diagonale + 1) as usize;
    let mut acc = vec![0u32; nb_rho * NB_THETA];
    let cos_sin: Vec<(f64, f64)> = (0..NB_THETA)
        .map(|t| {
            let angle = t as f64 * PI / NB_THETA as f64;
            (angle.cos(), angle.sin())
        })
        .collect();
    let mut nb_points = 0;
    for y in 0..g.hauteur {
        for x in 0..g.largeur {
            if g.valeurs[y * g.largeur + x] < seuil {
                continue;
            }
            nb_points += 1;
            for (t, (c, s)) in cos_sin.iter().enumerate() {
                let rho = (x as f64 * c + y as f64 * s).round() as i64;
                acc[(rho + diagonale) as usize * NB_THETA + t] += 1;
            }
        }
    }
    (Hough { diagonale, nb_rho, acc }, nb_points)
}

/// Les `n` droites les plus votées : (ρ, θ en degrés, votes). Après chaque choix, on efface
/// le voisinage du pic pour ne pas retrouver la même droite plusieurs fois.
fn pics(h: &Hough, n: usize) -> Vec<(i64, usize, u32)> {
    const VOISINAGE: i64 = 10;
    let mut acc = h.acc.clone();
    let mut resultat = Vec::new();
    for _ in 0..n {
        let (mut meilleur, mut votes) = (0usize, 0u32);
        for (i, &v) in acc.iter().enumerate() {
            if v > votes {
                votes = v;
                meilleur = i;
            }
        }
        if votes == 0 {
            break;
        }
        let rho = (meilleur / NB_THETA) as i64 - h.diagonale;
        let theta = meilleur % NB_THETA;
        resultat.push((rho, theta, votes));
        for dt in -VOISINAGE..=VOISINAGE {
            // θ + 180° est la même droite que θ avec ρ changé de signe
            let t = theta as i64 + dt;
            let (t, signe) = if t < 0 {
                (t + NB_THETA as i64, -1)
            } else if t >= NB_THETA as i64 {
                (t - NB_THETA as i64, -1)
            } else {
                (t, 1)
            };
            for dr in -VOISINAGE..=VOISINAGE {
                let ir = signe * rho + dr + h.diagonale;
                if ir >= 0 && (ir as usize) < h.nb_rho {
                    acc[ir as usize * NB_THETA + t as usize] = 0;
                }
            }
        }
    }
    resultat
}

/// Image de l'espace de Hough : largeur = 180 (θ), hauteur = 2·diagonale+1 (ρ), plus clair = plus de votes.
fn hough_vers_image(h: &Hough) -> Image {
    let max = h.acc.iter().copied().max().unwrap_or(0).max(1) as f64;
    let pixels = h.acc.iter().map(|&v| (v as f64 / max * 255.0).round() as u8).collect();
    Image { largeur: NB_THETA, hauteur: h.nb_rho, canaux: 1, pixels }
}

/// `traitements hough <entrée> <sortie.pgm> <sortie.json> <seuil 1..255> <nombre de droites>`
/// L'entrée est une image de contours : les pixels ≥ seuil comptent comme points de contour.
pub fn executer_hough(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée", "sortie image", "sortie json", "seuil", "nombre de droites"])?;
    let g = Carte::depuis_image(&pnm::lire(&args[0])?);
    let seuil = lire_entier(&args[3], "seuil", 1, 255)?;
    let nb_droites = lire_entier(&args[4], "nombre de droites", 1, 50)? as usize;

    let (h, nb_points) = hough(&g, seuil as f64);
    pnm::ecrire(&args[1], &hough_vers_image(&h))?;

    let mut json = format!("{{\"valeurs\": {{\"points de contour\": {nb_points}");
    for (i, (rho, theta, votes)) in pics(&h, nb_droites).iter().enumerate() {
        json.push_str(&format!(
            ", \"droite {n} : rho (pixels)\": {rho}, \"droite {n} : theta (degrés)\": {theta}, \"droite {n} : votes\": {votes}",
            n = i + 1
        ));
    }
    json.push_str("}}\n");
    fs::write(&args[2], json)?;
    Ok(())
}

// ===========================================================================
// 5. Amincissement des contours (suppression des non-maxima)
// ===========================================================================

/// Un contour épais (plusieurs pixels de large) est aminci à 1 pixel : on ne garde un pixel que
/// si sa norme de gradient est la plus grande dans la direction du gradient (perpendiculaire au
/// contour). Les pixels gardés conservent leur norme (ramenée à [0, 255]), les autres passent à 0.
fn amincir(g: &Carte, op: Operateur) -> Image {
    let (gx, gy) = gradient(g, op);
    let n = norme(&gx, &gy);
    let max = n.max_abs();
    let mut pixels = vec![0u8; n.valeurs.len()];
    for y in 0..g.hauteur {
        for x in 0..g.largeur {
            let i = y * g.largeur + x;
            let v = n.valeurs[i];
            if v <= 0.0 {
                continue;
            }
            // Direction du gradient ramenée à 4 possibilités (0°, 45°, 90°, 135°)
            let mut a = gy.valeurs[i].atan2(gx.valeurs[i]).to_degrees();
            if a < 0.0 {
                a += 180.0;
            }
            let (dx, dy): (i64, i64) = if a < 22.5 || a >= 157.5 {
                (1, 0)
            } else if a < 67.5 {
                (1, 1)
            } else if a < 112.5 {
                (0, 1)
            } else {
                (-1, 1)
            };
            let (xi, yi) = (x as i64, y as i64);
            let devant = n.valeur(xi + dx, yi + dy);
            let derriere = n.valeur(xi - dx, yi - dy);
            // « > » d'un côté et « ≥ » de l'autre : si deux voisins sont à égalité, on en garde un seul
            if v >= devant && v > derriere {
                pixels[i] = (v / max * 255.0).round() as u8;
            }
        }
    }
    Image { largeur: g.largeur, hauteur: g.hauteur, canaux: 1, pixels }
}

/// `traitements amincissement <entrée> <sortie> <sobel|prewitt>`
pub fn executer_amincissement(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée", "sortie", "opérateur (sobel|prewitt)"])?;
    let g = Carte::depuis_image(&pnm::lire(&args[0])?);
    let op = Operateur::depuis_texte(&args[2])?;
    pnm::ecrire(&args[1], &amincir(&g, op))?;
    Ok(())
}

// ===========================================================================
// 6. Seuillage par hystérésis
// ===========================================================================

/// Deux seuils : un pixel ≥ `haut` est sûrement un contour ; un pixel entre `bas` et `haut` n'est
/// gardé que s'il est relié (8-connexité) à un contour sûr. Cela supprime les petits bouts isolés
/// tout en gardant les contours faibles qui prolongent un contour fort.
fn hysteresis(g: &Carte, bas: f64, haut: f64) -> Image {
    let (l, h) = (g.largeur, g.hauteur);
    let mut garde = vec![false; l * h];
    let mut pile: Vec<usize> = Vec::new();
    for i in 0..l * h {
        if g.valeurs[i] >= haut {
            garde[i] = true;
            pile.push(i);
        }
    }
    while let Some(i) = pile.pop() {
        let (x, y) = ((i % l) as i64, (i / l) as i64);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (xx, yy) = (x + dx, y + dy);
                if (dx == 0 && dy == 0) || xx < 0 || yy < 0 || xx >= l as i64 || yy >= h as i64 {
                    continue;
                }
                let j = yy as usize * l + xx as usize;
                if !garde[j] && g.valeurs[j] >= bas {
                    garde[j] = true;
                    pile.push(j);
                }
            }
        }
    }
    image_binaire(l, h, &garde)
}

/// `traitements hysteresis <entrée> <sortie> <seuil bas> <seuil haut>`
/// L'entrée est une image de « force » de contour (par exemple la norme du gradient).
pub fn executer_hysteresis(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée", "sortie", "seuil bas", "seuil haut"])?;
    let g = Carte::depuis_image(&pnm::lire(&args[0])?);
    let bas = lire_entier(&args[2], "seuil bas", 0, 255)?;
    let haut = lire_entier(&args[3], "seuil haut", 0, 255)?;
    if bas > haut {
        return Err(format!("le seuil bas ({bas}) doit être inférieur ou égal au seuil haut ({haut})").into());
    }
    pnm::ecrire(&args[1], &hysteresis(&g, bas as f64, haut as f64))?;
    Ok(())
}

// ===========================================================================
// 7. Fermeture des contours (suivi du chemin de gradient le plus fort)
// ===========================================================================

/// Les 8 directions, dans l'ordre d'un tour complet (de 45° en 45°).
const DIRECTIONS: [(i64, i64); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];

/// Un contour est parfois interrompu. Pour chaque EXTRÉMITÉ (pixel de contour avec un seul
/// voisin), on prolonge le contour dans la direction où il allait : à chaque pas on regarde les
/// 3 pixels « devant » (tout droit, légèrement à gauche, légèrement à droite) et on avance vers
/// celui dont la force de gradient est la plus grande. Si on rejoint un autre contour avant
/// `longueur_max` pas (et sans passer sous `seuil`), le chemin est ajouté ; sinon on l'abandonne.
fn fermer(contours: &Carte, force: &Carte, seuil: f64, longueur_max: usize) -> Result<Image, String> {
    if contours.largeur != force.largeur || contours.hauteur != force.hauteur {
        return Err("les deux images doivent avoir la même taille".to_string());
    }
    let (l, h) = (contours.largeur as i64, contours.hauteur as i64);
    let dedans = |x: i64, y: i64| x >= 0 && y >= 0 && x < l && y < h;
    let est_contour = |x: i64, y: i64| dedans(x, y) && contours.valeurs[(y * l + x) as usize] >= 128.0;
    let force_en = |x: i64, y: i64| force.valeurs[(y * l + x) as usize];

    let mut sortie: Vec<bool> = contours.valeurs.iter().map(|&v| v >= 128.0).collect();

    for y in 0..h {
        for x in 0..l {
            if !est_contour(x, y) {
                continue;
            }
            // Extrémité = un seul voisin de contour
            let voisins: Vec<usize> =
                (0..8).filter(|&k| est_contour(x + DIRECTIONS[k].0, y + DIRECTIONS[k].1)).collect();
            if voisins.len() != 1 {
                continue;
            }
            // On repart dans la direction opposée à celle du voisin
            let mut dir = (voisins[0] + 4) % 8;
            let (mut cx, mut cy) = (x, y);
            let mut chemin: Vec<(i64, i64)> = Vec::new();
            let mut ferme = false;

            for _ in 0..longueur_max {
                let candidats: Vec<usize> = [(dir + 7) % 8, dir, (dir + 1) % 8]
                    .into_iter()
                    .filter(|&k| dedans(cx + DIRECTIONS[k].0, cy + DIRECTIONS[k].1))
                    .collect();
                if candidats.is_empty() {
                    break;
                }
                // Un candidat appartient déjà à un contour : on a rejoint un autre contour
                if candidats.iter().any(|&k| est_contour(cx + DIRECTIONS[k].0, cy + DIRECTIONS[k].1)) {
                    ferme = true;
                    break;
                }
                // Sinon on avance vers le candidat de plus forte force
                let mut meilleur = candidats[0];
                for &k in &candidats {
                    if force_en(cx + DIRECTIONS[k].0, cy + DIRECTIONS[k].1)
                        > force_en(cx + DIRECTIONS[meilleur].0, cy + DIRECTIONS[meilleur].1)
                    {
                        meilleur = k;
                    }
                }
                let (nx, ny) = (cx + DIRECTIONS[meilleur].0, cy + DIRECTIONS[meilleur].1);
                if force_en(nx, ny) < seuil {
                    break;
                }
                chemin.push((nx, ny));
                cx = nx;
                cy = ny;
                dir = meilleur;
            }

            if ferme {
                for (px, py) in chemin {
                    sortie[(py * l + px) as usize] = true;
                }
            }
        }
    }
    Ok(image_binaire(contours.largeur, contours.hauteur, &sortie))
}

/// `traitements fermeture-contours <contours> <force> <sortie> <seuil 0..255> <longueur max>`
pub fn executer_fermeture(args: &[String]) -> Resultat {
    verifier_arguments(args, &["contours", "force du gradient", "sortie", "seuil", "longueur maximale"])?;
    let contours = Carte::depuis_image(&pnm::lire(&args[0])?);
    let force = Carte::depuis_image(&pnm::lire(&args[1])?);
    let seuil = lire_entier(&args[3], "seuil", 0, 255)?;
    let longueur = lire_entier(&args[4], "longueur maximale", 1, 500)? as usize;
    pnm::ecrire(&args[2], &fermer(&contours, &force, seuil as f64, longueur)?)?;
    Ok(())
}

// ===========================================================================
// Tests (résultats calculés à la main)
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Carte construite à partir de lignes de texte « 0 0 100 100 ».
    fn carte(lignes: &[&str]) -> Carte {
        let rangees: Vec<Vec<f64>> =
            lignes.iter().map(|l| l.split_whitespace().map(|v| v.parse().unwrap()).collect()).collect();
        Carte { largeur: rangees[0].len(), hauteur: rangees.len(), valeurs: rangees.concat() }
    }

    fn en(c: &Carte, x: usize, y: usize) -> f64 {
        c.valeurs[y * c.largeur + x]
    }

    /// Image 6×5 : sombre (0) à gauche, clair (100) à droite, marche entre les colonnes 2 et 3.
    fn marche() -> Carte {
        carte(&["0 0 0 100 100 100"; 5])
    }

    #[test]
    fn derivee_en_x_d_une_marche() {
        let d = derivee(&marche(), "x").unwrap();
        assert_eq!(en(&d, 2, 2), 100.0); // f(3) − f(2)
        assert_eq!(en(&d, 1, 2), 0.0);
        assert_eq!(en(&d, 5, 2), 0.0); // bord répliqué : pas de faux contour
        let dy = derivee(&marche(), "y").unwrap();
        assert!(dy.valeurs.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn sobel_sur_une_marche_calcule_a_la_main() {
        // Pixel (2, 2) : colonne droite 100·(1+2+1) = 400, colonne gauche 0  =>  gx = 400
        let (gx, gy) = gradient(&marche(), Operateur::Sobel);
        assert_eq!(en(&gx, 2, 2), 400.0);
        assert_eq!(en(&gx, 3, 2), 400.0);
        assert_eq!(en(&gx, 0, 2), 0.0);
        assert_eq!(en(&gx, 5, 2), 0.0);
        assert!(gy.valeurs.iter().all(|&v| v == 0.0));
        // Prewitt : 100·3 = 300
        assert_eq!(en(&gradient(&marche(), Operateur::Prewitt).0, 2, 2), 300.0);
        // Roberts : f(0,0) − f(1,1) = 0 − 100 = −100 (le gradient peut être négatif !)
        assert_eq!(en(&gradient(&marche(), Operateur::Roberts).0, 2, 2), -100.0);
    }

    #[test]
    fn image_uniforme_sans_contour() {
        let g = carte(&["50 50 50", "50 50 50", "50 50 50"]);
        for op in [Operateur::Roberts, Operateur::Prewitt, Operateur::Sobel] {
            let (gx, gy) = gradient(&g, op);
            assert_eq!(norme(&gx, &gy).max_abs(), 0.0);
        }
        assert_eq!(laplacien(&g, "4").unwrap().max_abs(), 0.0);
        assert_eq!(laplacien(&g, "8").unwrap().max_abs(), 0.0);
    }

    #[test]
    fn sorties_du_gradient() {
        let g = marche();
        let norme_img = gradient_vers_image(&g, Operateur::Sobel, "norme", 0).unwrap();
        assert_eq!(norme_img.pixels[2 * 6 + 2], 255);
        assert_eq!(norme_img.pixels[2 * 6], 0);
        let seuil = gradient_vers_image(&g, Operateur::Sobel, "seuillage", 128).unwrap();
        assert_eq!(&seuil.pixels[0..6], &[0, 0, 255, 255, 0, 0]);
        // direction : gradient vers la droite => angle 0 => (0 + π) / 2π × 255 = 127,5 -> 128 ; hors contour : 0
        let dir = gradient_vers_image(&g, Operateur::Sobel, "direction", 0).unwrap();
        assert_eq!(dir.pixels[2 * 6 + 2], 128);
        assert_eq!(dir.pixels[2 * 6], 0);
        assert!(gradient_vers_image(&g, Operateur::Sobel, "n'importe quoi", 0).is_err());
    }

    #[test]
    fn laplacien_d_un_point_isole() {
        let g = carte(&["0 0 0", "0 10 0", "0 0 0"]);
        let l4 = laplacien(&g, "4").unwrap();
        assert_eq!(en(&l4, 1, 1), -40.0); // 0 + 0 + 0 + 0 − 4×10
        assert_eq!(en(&l4, 1, 0), 10.0); // un voisin à 10, trois à 0 (bord répliqué)  =>  10 − 0
        let l8 = laplacien(&g, "8").unwrap();
        assert_eq!(en(&l8, 1, 1), -80.0);
        assert!(laplacien(&g, "6").is_err());
    }

    #[test]
    fn passage_par_zero_sur_une_marche() {
        // Laplacien d'une marche : valeur négative d'un côté, positive de l'autre, 0 ailleurs
        let l = laplacien(&marche(), "4").unwrap();
        assert_eq!(en(&l, 2, 2), 100.0);
        assert_eq!(en(&l, 3, 2), -100.0);
        let img = passages_par_zero(&l, 50);
        // Une seule colonne marquée par ligne, au niveau de la marche
        for y in 0..5 {
            let marques: Vec<usize> = (0..6).filter(|&x| img.pixels[y * 6 + x] == 255).collect();
            assert_eq!(marques.len(), 1, "ligne {y}");
            assert!(marques[0] == 2 || marques[0] == 3);
        }
    }

    #[test]
    fn hough_retrouve_une_droite_horizontale() {
        // Droite horizontale y = 10 sur une image 30×20  =>  θ = 90°, ρ = 10
        let mut g = Carte::nouvelle(30, 20);
        for x in 0..30 {
            g.fixer(x, 10, 255.0);
        }
        let (h, nb_points) = hough(&g, 128.0);
        assert_eq!(nb_points, 30);
        let p = pics(&h, 3);
        assert_eq!(p[0], (10, 90, 30));
        // Une 2e droite très différente est bien plus faible : le voisinage du pic a été effacé
        assert!(p.len() < 2 || p[1].2 < 30);
    }

    #[test]
    fn hough_retrouve_une_droite_verticale() {
        // Droite verticale x = 7  =>  θ = 0°, ρ = 7
        let mut g = Carte::nouvelle(20, 30);
        for y in 0..30 {
            g.fixer(7, y, 255.0);
        }
        let (h, _) = hough(&g, 128.0);
        assert_eq!(pics(&h, 1)[0], (7, 0, 30));
    }

    #[test]
    fn amincissement_garde_une_seule_colonne() {
        let img = amincir(&marche(), Operateur::Sobel);
        for y in 0..5 {
            let marques: Vec<usize> = (0..6).filter(|&x| img.pixels[y * 6 + x] > 0).collect();
            assert_eq!(marques, vec![2], "ligne {y}");
            assert_eq!(img.pixels[y * 6 + 2], 255);
        }
    }

    #[test]
    fn hysteresis_garde_ce_qui_est_relie_a_un_contour_fort() {
        // Ligne : 200 (fort), 100 et 100 (faibles reliés), 10 (trop faible), 100 (faible mais isolé)
        let g = carte(&["200 100 100 10 100"]);
        let img = hysteresis(&g, 50.0, 150.0);
        assert_eq!(img.pixels, vec![255, 255, 255, 0, 0]);
    }

    #[test]
    fn fermeture_comble_un_trou() {
        // Ligne y = 2 : contour aux colonnes 0..=3 et 6..=9, trou en 4 et 5 où la force est élevée
        let mut contours = Carte::nouvelle(12, 5);
        let mut force = Carte::nouvelle(12, 5);
        for x in 0..10 {
            force.fixer(x, 2, 200.0);
            if x != 4 && x != 5 {
                contours.fixer(x, 2, 255.0);
            }
        }
        let img = fermer(&contours, &force, 50.0, 10).unwrap();
        for x in 0..10 {
            assert_eq!(img.pixels[2 * 12 + x], 255, "colonne {x}");
        }
        assert_eq!(img.pixels[2 * 12 + 10], 0);
        // Si la force est trop faible dans le trou, il reste ouvert
        let mut faible = Carte::nouvelle(12, 5);
        for x in 0..10 {
            faible.fixer(x, 2, 10.0);
        }
        let img = fermer(&contours, &faible, 50.0, 10).unwrap();
        assert_eq!(img.pixels[2 * 12 + 4], 0);
    }

    #[test]
    fn fermeture_refuse_deux_tailles_differentes() {
        assert!(fermer(&Carte::nouvelle(3, 3), &Carte::nouvelle(4, 3), 0.0, 5).is_err());
    }
}
