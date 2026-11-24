//! Transformée de Fourier 2D : outils communs au lot « Fourier et filtrage spectral ».
//!
//! Ce module n'est pas une sous-commande : il regroupe ce dont ont besoin les traitements
//! `fft`, `fft-inverse`, `filtre-spectral`, `coupe-bande` et `rehaussement-spectral` :
//!   - les nombres complexes (la bibliothèque standard n'en a pas, et le projet interdit les dépendances) ;
//!   - la FFT 1D (Cooley-Tukey, base 2) et la FFT 2D (toutes les lignes, puis toutes les colonnes) ;
//!   - le complément à zéro jusqu'à une puissance de 2, et le recadrage après la transformée inverse ;
//!   - le recentrage du spectre (inversion des quadrants) ;
//!   - les gabarits de filtres H(u, v) ;
//!   - le retour des valeurs réelles dans [0, 255] (normalisation ou saturation).
//!
//! Conventions :
//! ```text
//! directe : F(u, v) = Σx Σy f(x, y) · exp(-2iπ (u·x/M + v·y/N))
//! inverse : f(x, y) = 1/(M·N) · Σu Σv F(u, v) · exp(+2iπ (u·x/M + v·y/N))
//! ```
//! avec M × N la taille complétée (puissances de 2). Dans le tableau non recentré, la case u
//! correspond à la fréquence u pour u < M/2 et à u - M au-delà (fréquences négatives repliées à la fin).
//! Après recentrage, la fréquence (0, 0) est au pixel (M/2, N/2) : un pic vu au pixel (x, y)
//! du spectre recentré a pour fréquence (x - M/2, y - N/2).

use crate::pnm::Image;
use std::f64::consts::PI;
use std::ops::{Add, Mul, Sub};

/// Taille maximale (après complément à zéro) acceptée pour la FFT : 4096 × 4096 complexes
/// occupent déjà 256 Mo. Au-delà, le traitement refuse l'image avec un message clair.
pub const TAILLE_MAX: usize = 4096;

/// En dessous de ce module, un coefficient est considéré comme nul (sa phase n'a pas de sens).
pub const MODULE_NEGLIGEABLE: f64 = 1e-9;

// ===========================================================================
// Nombres complexes
// ===========================================================================

/// Nombre complexe `re + i·im`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Complexe {
    pub re: f64,
    pub im: f64,
}

impl Complexe {
    pub const ZERO: Complexe = Complexe { re: 0.0, im: 0.0 };

    pub fn new(re: f64, im: f64) -> Complexe {
        Complexe { re, im }
    }

    /// Nombre réel (partie imaginaire nulle).
    pub fn reel(re: f64) -> Complexe {
        Complexe { re, im: 0.0 }
    }

    /// `module · exp(i·phase)`.
    pub fn polaire(module: f64, phase: f64) -> Complexe {
        Complexe { re: module * phase.cos(), im: module * phase.sin() }
    }

    /// |z| = √(re² + im²)
    pub fn module(self) -> f64 {
        self.re.hypot(self.im)
    }

    /// Argument de z, dans [-π, π]. Vaut 0 pour un coefficient négligeable.
    pub fn phase(self) -> f64 {
        if self.module() < MODULE_NEGLIGEABLE {
            0.0
        } else {
            self.im.atan2(self.re)
        }
    }
}

impl Add for Complexe {
    type Output = Complexe;
    fn add(self, autre: Complexe) -> Complexe {
        Complexe::new(self.re + autre.re, self.im + autre.im)
    }
}

impl Sub for Complexe {
    type Output = Complexe;
    fn sub(self, autre: Complexe) -> Complexe {
        Complexe::new(self.re - autre.re, self.im - autre.im)
    }
}

impl Mul for Complexe {
    type Output = Complexe;
    fn mul(self, autre: Complexe) -> Complexe {
        Complexe::new(self.re * autre.re - self.im * autre.im, self.re * autre.im + self.im * autre.re)
    }
}

/// Multiplication par un réel (application d'un gain H(u, v)).
impl Mul<f64> for Complexe {
    type Output = Complexe;
    fn mul(self, k: f64) -> Complexe {
        Complexe::new(self.re * k, self.im * k)
    }
}

// ===========================================================================
// FFT 1D et 2D
// ===========================================================================

/// FFT 1D en place, algorithme de Cooley-Tukey en base 2 (version itérative).
/// La longueur doit être une puissance de 2. `inverse` : transformée inverse, division par n comprise.
///
/// 1. On range les échantillons dans l'ordre « bit-reverse » (indice écrit à l'envers en binaire).
/// 2. On combine des blocs de taille 2, 4, 8… avec les « papillons » :
///    a' = a + w·b,  b' = a - w·b,  avec w = exp(∓2iπ k / taille).
pub fn fft_1d(donnees: &mut [Complexe], inverse: bool) {
    let n = donnees.len();
    assert!(n.is_power_of_two(), "la FFT demande une longueur puissance de 2 (reçu {n})");
    if n == 1 {
        return;
    }

    // 1. Permutation bit-reverse
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            donnees.swap(i, j);
        }
    }

    // 2. Papillons, par blocs de taille croissante
    let signe = if inverse { 1.0 } else { -1.0 };
    let mut taille = 2;
    while taille <= n {
        let moitie = taille / 2;
        // facteurs w_k = exp(signe · 2iπ k / taille), calculés une fois par étage
        let facteurs: Vec<Complexe> =
            (0..moitie).map(|k| Complexe::polaire(1.0, signe * 2.0 * PI * k as f64 / taille as f64)).collect();
        for debut in (0..n).step_by(taille) {
            for (k, &w) in facteurs.iter().enumerate() {
                let a = donnees[debut + k];
                let b = donnees[debut + k + moitie] * w;
                donnees[debut + k] = a + b;
                donnees[debut + k + moitie] = a - b;
            }
        }
        taille *= 2;
    }

    if inverse {
        let facteur = 1.0 / n as f64;
        for z in donnees.iter_mut() {
            *z = *z * facteur;
        }
    }
}

/// FFT 2D en place (séparabilité) : FFT 1D de chaque ligne, puis de chaque colonne.
/// `valeurs` est rangé ligne par ligne : la case (u, v) est à l'indice `v * largeur + u`.
fn fft_2d(valeurs: &mut [Complexe], largeur: usize, hauteur: usize, inverse: bool) {
    for ligne in valeurs.chunks_exact_mut(largeur) {
        fft_1d(ligne, inverse);
    }
    let mut colonne = vec![Complexe::ZERO; hauteur];
    for x in 0..largeur {
        for y in 0..hauteur {
            colonne[y] = valeurs[y * largeur + x];
        }
        fft_1d(&mut colonne, inverse);
        for y in 0..hauteur {
            valeurs[y * largeur + x] = colonne[y];
        }
    }
}

// ===========================================================================
// Spectre d'une image
// ===========================================================================

/// Spectre F(u, v) d'une image en niveaux de gris, NON recentré.
#[derive(Debug, Clone)]
pub struct Spectre {
    /// Taille complétée (puissances de 2).
    pub largeur: usize,
    pub hauteur: usize,
    /// Taille de l'image de départ, pour recadrer après la transformée inverse.
    pub largeur_origine: usize,
    pub hauteur_origine: usize,
    /// F(u, v) à l'indice `v * largeur + u`.
    pub valeurs: Vec<Complexe>,
}

impl Spectre {
    /// Complète l'image avec des zéros jusqu'à une puissance de 2 dans chaque direction,
    /// puis calcule sa FFT 2D.
    pub fn depuis_image(image: &Image) -> Result<Spectre, String> {
        if image.est_couleur() {
            return Err("une image en niveaux de gris (PGM) est attendue".into());
        }
        let largeur = image.largeur.next_power_of_two();
        let hauteur = image.hauteur.next_power_of_two();
        if largeur > TAILLE_MAX || hauteur > TAILLE_MAX {
            return Err(format!(
                "image trop grande : {}x{} (complétée en {largeur}x{hauteur}), la FFT est limitée à {TAILLE_MAX}x{TAILLE_MAX}",
                image.largeur, image.hauteur
            ));
        }

        // Complément à zéro : l'image est copiée en haut à gauche, le reste vaut 0
        let mut valeurs = vec![Complexe::ZERO; largeur * hauteur];
        for y in 0..image.hauteur {
            for x in 0..image.largeur {
                valeurs[y * largeur + x] = Complexe::reel(image.valeur(x, y, 0) as f64);
            }
        }
        fft_2d(&mut valeurs, largeur, hauteur, false);

        Ok(Spectre { largeur, hauteur, largeur_origine: image.largeur, hauteur_origine: image.hauteur, valeurs })
    }

    /// Transformée inverse, puis recadrage à la taille d'origine (on enlève le complément à zéro).
    /// Renvoie la partie réelle, ligne par ligne. Elle n'est pas ramenée dans [0, 255] :
    /// utiliser ensuite [`saturer`] ou [`normaliser`].
    pub fn image_reconstruite(&self) -> Vec<f64> {
        let mut valeurs = self.valeurs.clone();
        fft_2d(&mut valeurs, self.largeur, self.hauteur, true);
        let mut sortie = Vec::with_capacity(self.largeur_origine * self.hauteur_origine);
        for y in 0..self.hauteur_origine {
            for x in 0..self.largeur_origine {
                // Pour un filtre symétrique (H(-u, -v) = H(u, v)), la partie imaginaire est nulle
                // (aux erreurs d'arrondi près) : on la laisse de côté.
                sortie.push(valeurs[y * self.largeur + x].re);
            }
        }
        sortie
    }

    /// Fréquence signée (fu, fv) de la case (u, v) du tableau non recentré :
    /// c'est la position de la case par rapport au centre du spectre recentré.
    pub fn frequence(&self, u: usize, v: usize) -> (f64, f64) {
        let signee = |i: usize, n: usize| ((i + n / 2) % n) as f64 - (n / 2) as f64;
        (signee(u, self.largeur), signee(v, self.hauteur))
    }

    /// Filtrage : F'(u, v) = F(u, v) · H(fu, fv), avec (fu, fv) la fréquence signée de la case.
    pub fn filtrer(&mut self, h: impl Fn(f64, f64) -> f64) {
        for v in 0..self.hauteur {
            for u in 0..self.largeur {
                let (fu, fv) = self.frequence(u, v);
                let i = v * self.largeur + u;
                self.valeurs[i] = self.valeurs[i] * h(fu, fv);
            }
        }
    }
}

/// Recentrage (inversion des quadrants, « fftshift ») : la case (0, 0) passe au centre
/// (largeur/2, hauteur/2). Avec des tailles paires, l'appliquer deux fois redonne le tableau de départ.
pub fn recentrer<T: Copy>(valeurs: &[T], largeur: usize, hauteur: usize) -> Vec<T> {
    let (dx, dy) = (largeur / 2, hauteur / 2);
    let mut sortie = valeurs.to_vec();
    for y in 0..hauteur {
        for x in 0..largeur {
            sortie[((y + dy) % hauteur) * largeur + (x + dx) % largeur] = valeurs[y * largeur + x];
        }
    }
    sortie
}

// ===========================================================================
// Gabarits de filtres
// ===========================================================================

/// Forme du gabarit d'un filtre passe-bas / passe-haut.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Forme {
    /// Coupure franche : gain 1 jusqu'à D0, 0 au-delà (provoque des oscillations, l'effet de Gibbs).
    Ideal,
    /// Transition douce : H = 1 / (1 + (D/D0)^(2n)).
    Butterworth { ordre: i32 },
    /// Transition sans oscillation : H = exp(-D² / (2·D0²)).
    Gaussien,
}

impl Forme {
    /// Lit la forme reçue en paramètre (`ideal`, `butterworth` ou `gaussien`).
    /// L'ordre ne sert que pour Butterworth.
    pub fn lire(texte: &str, ordre: i32) -> Result<Forme, String> {
        match texte {
            "ideal" => Ok(Forme::Ideal),
            "gaussien" => Ok(Forme::Gaussien),
            "butterworth" if ordre >= 1 => Ok(Forme::Butterworth { ordre }),
            "butterworth" => Err(format!("l'ordre du filtre de Butterworth doit être ≥ 1 (reçu {ordre})")),
            autre => Err(format!("forme inconnue : « {autre} » (ideal, butterworth ou gaussien)")),
        }
    }

    /// Gain passe-bas à la distance `d` du centre du spectre, pour une fréquence de coupure `d0` > 0.
    pub fn passe_bas(self, d: f64, d0: f64) -> f64 {
        match self {
            Forme::Ideal => {
                if d <= d0 {
                    1.0
                } else {
                    0.0
                }
            }
            Forme::Butterworth { ordre } => 1.0 / (1.0 + (d / d0).powi(2 * ordre)),
            Forme::Gaussien => (-(d * d) / (2.0 * d0 * d0)).exp(),
        }
    }

    /// Gain passe-haut : complément du passe-bas, H_ph = 1 - H_pb.
    pub fn passe_haut(self, d: f64, d0: f64) -> f64 {
        1.0 - self.passe_bas(d, d0)
    }
}

// ===========================================================================
// Retour dans [0, 255] et lecture des paramètres
// ===========================================================================

/// Normalisation min-max : la plus petite valeur devient 0, la plus grande 255.
/// Une image constante (étendue négligeable) donne une image noire.
pub fn normaliser(valeurs: &[f64], largeur: usize, hauteur: usize) -> Image {
    let min = valeurs.iter().copied().fold(f64::INFINITY, f64::min);
    let max = valeurs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let etendue = max - min;
    let pixels = valeurs
        .iter()
        .map(|&v| if etendue > 1e-6 { ((v - min) / etendue * 255.0).round() as u8 } else { 0 })
        .collect();
    Image::depuis_pixels(largeur, hauteur, 1, pixels).expect("une valeur par pixel")
}

/// Saturation : arrondi, puis tout ce qui sort de [0, 255] est ramené à la borne la plus proche.
pub fn saturer(valeurs: &[f64], largeur: usize, hauteur: usize) -> Image {
    let pixels = valeurs.iter().map(|&v| v.round().clamp(0.0, 255.0) as u8).collect();
    Image::depuis_pixels(largeur, hauteur, 1, pixels).expect("une valeur par pixel")
}

/// Lit un paramètre numérique, avec un message clair s'il est invalide.
pub fn lire_nombre<T: std::str::FromStr>(texte: &str, nom: &str) -> Result<T, String> {
    texte.trim().parse().map_err(|_| format!("{nom} invalide : « {texte} » (nombre attendu)"))
}

/// Lit un paramètre booléen (`1` ou `0`, comme l'envoie le serveur).
pub fn lire_booleen(texte: &str, nom: &str) -> Result<bool, String> {
    match texte {
        "1" => Ok(true),
        "0" => Ok(false),
        autre => Err(format!("{nom} invalide : « {autre} » (1 ou 0 attendu)")),
    }
}

// ===========================================================================
// Tests (cargo test)
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn proche(a: Complexe, b: Complexe) -> bool {
        (a.re - b.re).abs() < 1e-9 && (a.im - b.im).abs() < 1e-9
    }

    fn reels(valeurs: &[f64]) -> Vec<Complexe> {
        valeurs.iter().map(|&v| Complexe::reel(v)).collect()
    }

    /// DFT calculée directement avec la définition (lente, sert de référence).
    fn dft_naive(entree: &[Complexe]) -> Vec<Complexe> {
        let n = entree.len();
        (0..n)
            .map(|u| {
                entree.iter().enumerate().fold(Complexe::ZERO, |somme, (x, &f)| {
                    somme + f * Complexe::polaire(1.0, -2.0 * PI * (u * x) as f64 / n as f64)
                })
            })
            .collect()
    }

    #[test]
    fn fft_d_une_impulsion_vaut_1_partout() {
        let mut donnees = reels(&[1.0, 0.0, 0.0, 0.0]);
        fft_1d(&mut donnees, false);
        assert!(donnees.iter().all(|&z| proche(z, Complexe::reel(1.0))));
    }

    #[test]
    fn fft_calculee_a_la_main() {
        // [1, 2, 3, 4] : F(0) = 10, F(1) = -2 + 2i, F(2) = -2, F(3) = -2 - 2i
        let mut donnees = reels(&[1.0, 2.0, 3.0, 4.0]);
        fft_1d(&mut donnees, false);
        let attendu = [Complexe::new(10.0, 0.0), Complexe::new(-2.0, 2.0), Complexe::new(-2.0, 0.0), Complexe::new(-2.0, -2.0)];
        for (z, a) in donnees.iter().zip(attendu) {
            assert!(proche(*z, a), "{z:?} au lieu de {a:?}");
        }
    }

    #[test]
    fn fft_egale_la_definition() {
        let entree: Vec<Complexe> = (0..16).map(|i| Complexe::new((i * 7 % 5) as f64, (i % 3) as f64)).collect();
        let mut donnees = entree.clone();
        fft_1d(&mut donnees, false);
        for (z, a) in donnees.iter().zip(dft_naive(&entree)) {
            assert!(proche(*z, a));
        }
    }

    #[test]
    fn la_fft_inverse_redonne_le_signal() {
        let entree = reels(&[3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0]);
        let mut donnees = entree.clone();
        fft_1d(&mut donnees, false);
        fft_1d(&mut donnees, true);
        assert!(donnees.iter().zip(&entree).all(|(a, b)| proche(*a, *b)));
    }

    #[test]
    fn la_composante_continue_est_la_somme_des_pixels() {
        let image = Image::depuis_pixels(2, 2, 1, vec![10, 20, 30, 40]).unwrap();
        let spectre = Spectre::depuis_image(&image).unwrap();
        assert!(proche(spectre.valeurs[0], Complexe::reel(100.0)));
    }

    #[test]
    fn complement_a_zero_puis_recadrage() {
        let image = Image::depuis_pixels(3, 5, 1, (0..15).map(|v| v * 10).collect()).unwrap();
        let spectre = Spectre::depuis_image(&image).unwrap();
        assert_eq!((spectre.largeur, spectre.hauteur), (4, 8));
        let retour = saturer(&spectre.image_reconstruite(), 3, 5);
        assert_eq!(retour, image);
    }

    #[test]
    fn refuse_une_image_couleur() {
        let image = Image::nouvelle(2, 2, 3);
        assert!(Spectre::depuis_image(&image).is_err());
    }

    #[test]
    fn recentrage_par_inversion_des_quadrants() {
        // 4x2 : la case (0, 0) passe en (2, 1)
        let valeurs = [1, 2, 3, 4, 5, 6, 7, 8];
        let recentre = recentrer(&valeurs, 4, 2);
        assert_eq!(recentre, vec![7, 8, 5, 6, 3, 4, 1, 2]);
        assert_eq!(recentrer(&recentre, 4, 2), valeurs.to_vec());
    }

    #[test]
    fn frequences_signees() {
        let spectre = Spectre::depuis_image(&Image::nouvelle(8, 1, 1)).unwrap();
        let fu: Vec<f64> = (0..8).map(|u| spectre.frequence(u, 0).0).collect();
        assert_eq!(fu, vec![0.0, 1.0, 2.0, 3.0, -4.0, -3.0, -2.0, -1.0]);
        assert_eq!(spectre.frequence(0, 0), (0.0, 0.0)); // hauteur 1
    }

    #[test]
    fn gabarits_aux_points_cles() {
        assert_eq!(Forme::Ideal.passe_bas(10.0, 10.0), 1.0);
        assert_eq!(Forme::Ideal.passe_bas(10.5, 10.0), 0.0);
        assert!((Forme::Butterworth { ordre: 2 }.passe_bas(10.0, 10.0) - 0.5).abs() < 1e-12);
        assert!((Forme::Gaussien.passe_bas(10.0, 10.0) - (-0.5f64).exp()).abs() < 1e-12);
        assert_eq!(Forme::Gaussien.passe_haut(0.0, 10.0), 0.0);
    }

    #[test]
    fn lecture_des_parametres() {
        assert_eq!(Forme::lire("butterworth", 3), Ok(Forme::Butterworth { ordre: 3 }));
        assert!(Forme::lire("butterworth", 0).is_err());
        assert!(Forme::lire("carre", 1).is_err());
        assert_eq!(lire_booleen("1", "x"), Ok(true));
        assert!(lire_booleen("oui", "x").is_err());
        assert_eq!(lire_nombre::<f64>("2.5", "k"), Ok(2.5));
    }

    #[test]
    fn normalisation_et_saturation() {
        assert_eq!(normaliser(&[-1.0, 0.0, 1.0], 3, 1).pixels, vec![0, 128, 255]);
        assert_eq!(normaliser(&[7.0, 7.0], 2, 1).pixels, vec![0, 0]);
        assert_eq!(saturer(&[-3.0, 12.4, 300.0], 3, 1).pixels, vec![0, 12, 255]);
    }
}
