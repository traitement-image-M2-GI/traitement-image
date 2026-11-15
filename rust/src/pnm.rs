//! Lecture et écriture des images PGM (P5) et PPM (P6).
//!
//! C'est le format d'échange imposé par le contrat du projet (DEVELOPPEMENT.md) :
//! le serveur donne aux programmes des fichiers PGM/PPM et attend un PGM/PPM en retour.
//! Ce module est commun à tous les traitements écrits en Rust : aucun traitement ne doit
//! lire ou écrire un fichier image autrement qu'avec `pnm::lire` et `pnm::ecrire`.
//!
//! Format accepté (contrat) :
//! ```text
//! P5                  <- P5 = niveaux de gris (1 octet par pixel), P6 = couleur (3 octets : R, G, B)
//! # commentaire       <- facultatif, ignoré
//! <largeur> <hauteur>
//! 255                 <- valeur max : toujours 255 (8 bits)
//! <pixels>            <- ligne par ligne, de haut en bas, de gauche à droite
//! ```

// Certaines fonctions ne servent pas encore (elles sont là pour les autres traitements du groupe).
#![allow(dead_code)]

use std::fmt;
use std::fs;
use std::path::Path;

// ===========================================================================
// L'image en mémoire
// ===========================================================================

/// Une image 8 bits, en niveaux de gris (`canaux == 1`) ou en couleur RGB (`canaux == 3`).
///
/// Les pixels sont rangés dans un seul tableau, ligne par ligne. En couleur, les trois
/// canaux d'un pixel se suivent : `R G B R G B ...`.
/// Le pixel (x, y), canal c, est à l'indice `(y * largeur + x) * canaux + c` (voir [`Image::indice`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub largeur: usize,
    pub hauteur: usize,
    pub canaux: usize,
    pub pixels: Vec<u8>,
}

impl Image {
    /// Image noire (tous les pixels à 0) de la taille donnée.
    pub fn nouvelle(largeur: usize, hauteur: usize, canaux: usize) -> Image {
        assert!(canaux == 1 || canaux == 3, "une image a 1 canal (gris) ou 3 canaux (RGB), pas {canaux}");
        Image { largeur, hauteur, canaux, pixels: vec![0; largeur * hauteur * canaux] }
    }

    /// Image construite à partir d'un tableau de pixels déjà rempli (vérifie sa taille).
    pub fn depuis_pixels(largeur: usize, hauteur: usize, canaux: usize, pixels: Vec<u8>) -> Result<Image, ErreurPnm> {
        if canaux != 1 && canaux != 3 {
            return Err(ErreurPnm::Format(format!("nombre de canaux invalide : {canaux} (1 ou 3 attendu)")));
        }
        if pixels.len() != largeur * hauteur * canaux {
            return Err(ErreurPnm::Format(format!(
                "{} valeurs fournies pour une image {largeur}x{hauteur} à {canaux} canal(aux) ({} attendues)",
                pixels.len(),
                largeur * hauteur * canaux
            )));
        }
        Ok(Image { largeur, hauteur, canaux, pixels })
    }

    /// Vrai pour une image couleur (PPM).
    pub fn est_couleur(&self) -> bool {
        self.canaux == 3
    }

    /// Nombre de pixels (largeur × hauteur), sans compter les canaux.
    pub fn nb_pixels(&self) -> usize {
        self.largeur * self.hauteur
    }

    /// Position dans `pixels` du canal `c` du pixel (x, y).
    #[inline]
    pub fn indice(&self, x: usize, y: usize, c: usize) -> usize {
        debug_assert!(x < self.largeur && y < self.hauteur && c < self.canaux);
        (y * self.largeur + x) * self.canaux + c
    }

    /// Valeur du canal `c` du pixel (x, y). En gris, `c` vaut toujours 0.
    #[inline]
    pub fn valeur(&self, x: usize, y: usize, c: usize) -> u8 {
        self.pixels[self.indice(x, y, c)]
    }

    /// Modifie le canal `c` du pixel (x, y).
    #[inline]
    pub fn modifier(&mut self, x: usize, y: usize, c: usize, v: u8) {
        let i = self.indice(x, y, c);
        self.pixels[i] = v;
    }
}

// ===========================================================================
// Erreurs
// ===========================================================================

/// Erreur de lecture ou d'écriture. Son message est fait pour être écrit sur stderr :
/// le serveur le transmet tel quel à l'interface.
#[derive(Debug)]
pub enum ErreurPnm {
    /// Le fichier n'a pas pu être lu ou écrit (absent, droits, disque plein…).
    Fichier { chemin: String, source: std::io::Error },
    /// Le contenu n'est pas une image PGM/PPM conforme au contrat.
    Format(String),
}

impl fmt::Display for ErreurPnm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErreurPnm::Fichier { chemin, source } => write!(f, "{chemin} : {source}"),
            ErreurPnm::Format(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for ErreurPnm {}

// ===========================================================================
// Lecture / écriture de fichiers
// ===========================================================================

/// Lit un fichier PGM (P5) ou PPM (P6).
pub fn lire(chemin: impl AsRef<Path>) -> Result<Image, ErreurPnm> {
    let chemin = chemin.as_ref();
    let octets = fs::read(chemin).map_err(|source| ErreurPnm::Fichier { chemin: chemin.display().to_string(), source })?;
    decoder(&octets).map_err(|e| match e {
        ErreurPnm::Format(message) => ErreurPnm::Format(format!("{} : {message}", chemin.display())),
        autre => autre,
    })
}

/// Écrit l'image en PGM (P5) si elle a 1 canal, en PPM (P6) si elle en a 3.
pub fn ecrire(chemin: impl AsRef<Path>, image: &Image) -> Result<(), ErreurPnm> {
    let chemin = chemin.as_ref();
    fs::write(chemin, encoder(image)).map_err(|source| ErreurPnm::Fichier { chemin: chemin.display().to_string(), source })
}

// ===========================================================================
// Conversion octets <-> image
// ===========================================================================

/// Convertit le contenu d'un fichier PGM/PPM en [`Image`].
pub fn decoder(octets: &[u8]) -> Result<Image, ErreurPnm> {
    if octets.len() < 2 || octets[0] != b'P' {
        return Err(ErreurPnm::Format("ce n'est pas une image PGM/PPM (doit commencer par P5 ou P6)".into()));
    }
    let canaux = match octets[1] {
        b'5' => 1,
        b'6' => 3,
        autre => {
            return Err(ErreurPnm::Format(format!(
                "format P{} non supporté : seuls P5 (PGM binaire) et P6 (PPM binaire) sont acceptés",
                autre as char
            )))
        }
    };

    // En-tête : largeur, hauteur, valeur max (séparés par des blancs, commentaires possibles)
    let mut pos = 2;
    let largeur = lire_entier(octets, &mut pos)?;
    let hauteur = lire_entier(octets, &mut pos)?;
    let maxval = lire_entier(octets, &mut pos)?;

    // Exactement UN caractère blanc sépare la valeur max des pixels
    if pos >= octets.len() || !octets[pos].is_ascii_whitespace() {
        return Err(ErreurPnm::Format("en-tête invalide : un blanc est attendu après la valeur max".into()));
    }
    pos += 1;

    if largeur == 0 || hauteur == 0 {
        return Err(ErreurPnm::Format(format!("dimensions invalides : {largeur}x{hauteur}")));
    }
    if maxval != 255 {
        return Err(ErreurPnm::Format(format!("valeur max {maxval} non supportée : elle doit être 255 (8 bits)")));
    }

    let attendu = largeur
        .checked_mul(hauteur)
        .and_then(|n| n.checked_mul(canaux))
        .ok_or_else(|| ErreurPnm::Format(format!("dimensions trop grandes : {largeur}x{hauteur}")))?;
    let disponible = octets.len() - pos;
    if disponible < attendu {
        return Err(ErreurPnm::Format(format!("données tronquées : {disponible} octets de pixels, {attendu} attendus")));
    }

    Ok(Image { largeur, hauteur, canaux, pixels: octets[pos..pos + attendu].to_vec() })
}

/// Convertit une [`Image`] en contenu de fichier PGM (P5) ou PPM (P6).
pub fn encoder(image: &Image) -> Vec<u8> {
    let magique = if image.canaux == 1 { "P5" } else { "P6" };
    let mut sortie = format!("{magique}\n{} {}\n255\n", image.largeur, image.hauteur).into_bytes();
    sortie.extend_from_slice(&image.pixels);
    sortie
}

/// Lit le prochain entier de l'en-tête à partir de `pos`, en sautant blancs et commentaires (`# ...`).
fn lire_entier(octets: &[u8], pos: &mut usize) -> Result<usize, ErreurPnm> {
    loop {
        while *pos < octets.len() && octets[*pos].is_ascii_whitespace() {
            *pos += 1;
        }
        if *pos < octets.len() && octets[*pos] == b'#' {
            while *pos < octets.len() && octets[*pos] != b'\n' && octets[*pos] != b'\r' {
                *pos += 1;
            }
        } else {
            break;
        }
    }
    let debut = *pos;
    while *pos < octets.len() && octets[*pos].is_ascii_digit() {
        *pos += 1;
    }
    if debut == *pos {
        return Err(ErreurPnm::Format("en-tête PGM/PPM incomplet ou invalide".into()));
    }
    // que des chiffres ASCII : la conversion en texte ne peut pas échouer
    std::str::from_utf8(&octets[debut..*pos])
        .unwrap()
        .parse::<usize>()
        .map_err(|_| ErreurPnm::Format("nombre trop grand dans l'en-tête".into()))
}

// ===========================================================================
// Tests (cargo test)
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aller_retour_gris() {
        let image = Image::depuis_pixels(3, 2, 1, vec![0, 10, 20, 30, 40, 255]).unwrap();
        let octets = encoder(&image);
        assert!(octets.starts_with(b"P5\n3 2\n255\n"));
        assert_eq!(decoder(&octets).unwrap(), image);
    }

    #[test]
    fn aller_retour_couleur() {
        let image = Image::depuis_pixels(2, 1, 3, vec![255, 0, 0, 0, 0, 255]).unwrap();
        let octets = encoder(&image);
        assert!(octets.starts_with(b"P6\n2 1\n255\n"));
        assert_eq!(decoder(&octets).unwrap(), image);
    }

    #[test]
    fn commentaires_et_blancs_dans_l_en_tete() {
        let octets = b"P5\n# cree par GIMP\n2  1\n# encore\n255\n\x07\x08";
        let image = decoder(octets).unwrap();
        assert_eq!((image.largeur, image.hauteur, image.canaux), (2, 1, 1));
        assert_eq!(image.pixels, vec![7, 8]);
    }

    #[test]
    fn un_pixel_valant_un_blanc_n_est_pas_saute() {
        // 0x0A ('\n') comme premier pixel : seul UN blanc doit être consommé après 255
        let image = decoder(b"P5\n2 1\n255\n\n\x0A").unwrap();
        assert_eq!(image.pixels, vec![10, 10]);
    }

    #[test]
    fn indice_ligne_par_ligne() {
        let mut image = Image::nouvelle(4, 3, 3);
        image.modifier(1, 2, 2, 99); // x = 1, y = 2, canal B
        assert_eq!(image.indice(1, 2, 2), (2 * 4 + 1) * 3 + 2);
        assert_eq!(image.valeur(1, 2, 2), 99);
    }

    #[test]
    fn refuse_les_formats_hors_contrat() {
        assert!(decoder(b"P2\n1 1\n255\n0").is_err()); // PGM ASCII
        assert!(decoder(b"P5\n1 1\n65535\n\0\0").is_err()); // 16 bits
        assert!(decoder(b"P5\n4 4\n255\n\0").is_err()); // tronquée
        assert!(decoder(b"P5\n0 4\n255\n").is_err()); // dimension nulle
        assert!(decoder(b"\x89PNG").is_err()); // pas du PGM/PPM
    }

    #[test]
    fn message_d_erreur_lisible() {
        let message = decoder(b"P3\n1 1\n255\n0 0 0").unwrap_err().to_string();
        assert!(message.contains("P3"), "{message}");
    }
}
