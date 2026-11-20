//! Traitements sur les images binaires (cours 07 « Images binaires »).
//!
//! Ce module contient les outils communs aux traitements binaires (vérification du
//! format 0/255) et les cartes de distance discrètes (diapos 6 et 7 du cours) :
//!   - D4 (Manhattan) : d(p, g) = |x - gx| + |y - gy|   -> forme un losange
//!   - D8 (échiquier) : d(p, g) = max(|x - gx|, |y - gy|) -> forme un carré
//!
//! La distance est calculée pour les pixels objet (255) ; le fond (0) reste à 0.
//! Les distances supérieures à 255 sont ramenées à 255, comme l'impose le contrat.

use crate::pnm::{self, Image};
use crate::{verifier_arguments, Resultat};
use std::fs;

// ===========================================================================
// Outils communs
// ===========================================================================

/// Vérifie que l'image est un PGM binaire : 1 canal, pixels valant 0 ou 255.
/// Le serveur validation déjà le type `binaire`, mais le programme doit aussi
/// pouvoir être lancé seul en ligne de commande.
pub fn verifier_binaire(image: &Image) -> Resultat {
    if image.est_couleur() {
        return Err("une image binaire PGM (P5) est attendue, pas une image couleur".into());
    }
    if let Some(valeur) = image.pixels.iter().find(|&&v| v != 0 && v != 255) {
        return Err(format!(
            "image binaire invalide : pixel de valeur {valeur} (seuls 0 et 255 sont autorisés)"
        )
        .into());
    }
    Ok(())
}

/// Vrai si le pixel (x, y) appartient à l'objet ; faux au bord ou si c'est le fond.
/// Les coordonnées sont en `i64` pour pouvoir tester x - 1 quand x vaut 0.
pub fn est_objet(image: &Image, x: i64, y: i64) -> bool {
    x >= 0
        && y >= 0
        && x < image.largeur as i64
        && y < image.hauteur as i64
        && image.valeur(x as usize, y as usize, 0) == 255
}

/// Connexité utilisée par le codage de Freeman et par l'étiquetage.
///
/// Cours 07, diapo 4 : la connexité-4 sert pour les régions, la connexité-8 pour les contours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connexite {
    Quatre,
    Huit,
}

impl Connexite {
    pub fn depuis_chaine(texte: &str) -> Result<Connexite, String> {
        match texte {
            "4" => Ok(Connexite::Quatre),
            "8" => Ok(Connexite::Huit),
            autre => Err(format!("connexité inconnue : « {autre} » (4 ou 8 attendu)")),
        }
    }

    /// Voisins déjà rencontrés lors du parcours rows-major (haut, gauche, et les deux
    /// coins en 8-connexité). Utilisés au premier passage de l'étiquetage.
    pub fn voisins_precedents(self) -> &'static [(i64, i64)] {
        match self {
            Connexite::Quatre => &[(0, -1), (-1, 0)],
            Connexite::Huit => &[(0, -1), (-1, -1), (1, -1), (-1, 0)],
        }
    }

    /// Codes de Freeman (diapo 8) dans l'ordre de recherche horaire.
    /// En 8-connexité : 0 = Est, 1 = Sud-Est, 2 = Sud, … 7 = Nord-Est.
    /// En 4-connexité, les codes du cours sont 0 = Est, 1 = Nord, 2 = Ouest, 3 = Sud,
    /// d'où l'ordre de balayage 0, 3, 2, 1.
    pub fn directions_freeman(self) -> Vec<(u8, i64, i64)> {
        match self {
            Connexite::Huit => vec![
                (0, 1, 0), (1, 1, 1), (2, 0, 1), (3, -1, 1),
                (4, -1, 0), (5, -1, -1), (6, 0, -1), (7, 1, -1),
            ],
            Connexite::Quatre => vec![(0, 1, 0), (3, 0, 1), (2, -1, 0), (1, 0, -1)],
        }
    }
}

// ===========================================================================
// Cartes de distance
// ===========================================================================

/// Méthode de distance discrète, choisie par l'utilisateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Distance {
    D4,
    D8,
}

impl Distance {
    /// Lit la méthode depuis l'argument texte reçu sur la ligne de commande.
    pub fn depuis_chaine(texte: &str) -> Result<Distance, String> {
        match texte {
            "D4" => Ok(Distance::D4),
            "D8" => Ok(Distance::D8),
            autre => Err(format!("distance inconnue : « {autre} » (D4 ou D8 attendu)")),
        }
    }

    fn entre(self, dx: usize, dy: usize) -> usize {
        match self {
            Distance::D4 => dx + dy,
            Distance::D8 => dx.max(dy),
        }
    }
}

/// Carte de distance au germe `(germe_x, germe_y)`.
///
/// `abs_diff` évite le débordement d'un `usize` quand x < germe_x (0 - 1 planterait).
pub fn carte_distance(image: &Image, methode: Distance, germe_x: usize, germe_y: usize) -> Image {
    let mut sortie = Image::nouvelle(image.largeur, image.hauteur, 1);

    for y in 0..image.hauteur {
        for x in 0..image.largeur {
            if image.valeur(x, y, 0) == 0 {
                continue; // le fond reste noir
            }
            let d = methode.entre(x.abs_diff(germe_x), y.abs_diff(germe_y));
            sortie.modifier(x, y, 0, d.min(255) as u8);
        }
    }
    sortie
}

/// `traitements distances <entrée.pgm> <sortie.pgm> <D4|D8> <germe x> <germe y>`
pub fn executer_distances(args: &[String]) -> Resultat {
    verifier_arguments(
        args,
        &["entrée.pgm", "sortie.pgm", "distance (D4|D8)", "germe x", "germe y"],
    )?;
    let image = pnm::lire(&args[0])?;
    verifier_binaire(&image)?;
    let methode = Distance::depuis_chaine(&args[2])?;
    let germe_x: usize = args[3]
        .parse()
        .map_err(|_| format!("germe x invalide : « {} » (entier attendu)", args[3]))?;
    let germe_y: usize = args[4]
        .parse()
        .map_err(|_| format!("germe y invalide : « {} » (entier attendu)", args[4]))?;

    if germe_x >= image.largeur || germe_y >= image.hauteur {
        return Err(format!(
            "germe ({germe_x}, {germe_y}) hors de l'image ({} x {})",
            image.largeur, image.hauteur
        )
        .into());
    }

    pnm::ecrire(&args[1], &carte_distance(&image, methode, germe_x, germe_y))?;
    Ok(())
}

// ===========================================================================
// Étiquetage des composantes connexes
// ===========================================================================

/// Table d'équivalence (union-find) du premier parcours : dès qu'un pixel touche
/// deux étiquettes provisoires, on note qu'elles appartiennent à la même composante.
struct Equivalences {
    parent: Vec<usize>,
    taille: Vec<usize>,
}

impl Equivalences {
    fn nouvelle() -> Self {
        Self { parent: vec![0], taille: vec![0] } // l'indice 0 est réservé au fond
    }

    /// Crée une étiquette provisoire et renvoie son numéro.
    fn ajouter(&mut self) -> usize {
        let etiquette = self.parent.len();
        self.parent.push(etiquette);
        self.taille.push(1);
        etiquette
    }

    /// Représentant de la classe, avec compression du chemin.
    fn representant(&mut self, mut etiquette: usize) -> usize {
        while self.parent[etiquette] != etiquette {
            self.parent[etiquette] = self.parent[self.parent[etiquette]];
            etiquette = self.parent[etiquette];
        }
        etiquette
    }

    fn unir(&mut self, a: usize, b: usize) {
        let (mut ra, mut rb) = (self.representant(a), self.representant(b));
        if ra == rb {
            return;
        }
        if self.taille[ra] < self.taille[rb] {
            std::mem::swap(&mut ra, &mut rb);
        }
        self.parent[rb] = ra;
        self.taille[ra] += self.taille[rb];
    }
}

/// Couleur d'une étiquette (le fond reste noir).
///
/// La multiplication par un nombre impair est une bijection modulo 2^24 : deux
/// étiquettes différentes reçoivent donc toujours deux couleurs différentes, et
/// aucune n'est noire (le code 0 n'est atteint que par l'étiquette 0, inutilisée).
fn couleur_etiquette(etiquette: usize) -> [u8; 3] {
    let code = (etiquette as u32).wrapping_mul(0x009E_3779) & 0x00FF_FFFF;
    [(code >> 16) as u8, (code >> 8) as u8, code as u8]
}

/// Étiquetage en deux passes (cours 07, diapos 10 à 35).
///
/// 1. on parcourt l'image ligne par ligne : chaque pixel objet reçoit la plus
///    petite étiquette de ses voisins déjà vus (haut, gauche et éventuellement
///    les deux coins en 8-connexité), ou une nouvelle, et on fusionne les
///    étiquettes voisines dans la table d'équivalence ;
/// 2. on remplace chaque étiquette provisoire par son représentant, numéroté
///    de 1 à n, et on écrit une image couleur (une couleur par composante).
///
/// Renvoie l'image colorée et le nombre de pixels par composante (`tailles[0]`
/// est inutilisé, les composantes sont numérotées à partir de 1).
pub fn etiqueter_composantes(image: &Image, connexite: Connexite) -> (Image, Vec<usize>) {
    let voisins = connexite.voisins_precedents();
    let mut equivalences = Equivalences::nouvelle();
    let mut provisoires = vec![0usize; image.nb_pixels()];

    // Premier parcours : étiquettes provisoires + unions.
    for y in 0..image.hauteur {
        for x in 0..image.largeur {
            let i = y * image.largeur + x;
            if image.valeur(x, y, 0) == 0 {
                continue; // le fond garde l'étiquette 0
            }

            let mut plus_petit = usize::MAX;
            let mut vus = Vec::new();
            for &(dx, dy) in voisins {
                let vx = x as i64 + dx;
                let vy = y as i64 + dy;
                if vx < 0 || vy < 0 || vx >= image.largeur as i64 || vy >= image.hauteur as i64 {
                    continue;
                }
                let etiquette = provisoires[vy as usize * image.largeur + vx as usize];
                if etiquette != 0 {
                    plus_petit = plus_petit.min(etiquette);
                    vus.push(etiquette);
                }
            }

            let etiquette = if plus_petit == usize::MAX {
                equivalences.ajouter()
            } else {
                plus_petit
            };
            provisoires[i] = etiquette;
            for vue in vus {
                equivalences.unir(etiquette, vue);
            }
        }
    }

    // Deuxième parcours : numérotation compacte 1..=n et comptage des pixels.
    let mut compactes = vec![0usize; equivalences.parent.len()];
    let mut tailles: Vec<usize> = vec![0];
    for &provisoire in &provisoires {
        if provisoire == 0 {
            continue;
        }
        let racine = equivalences.representant(provisoire);
        if compactes[racine] == 0 {
            compactes[racine] = tailles.len();
            tailles.push(0);
        }
        tailles[compactes[racine]] += 1;
    }

    // Image de sortie : une couleur par composante, le fond restant noir.
    let couleurs: Vec<[u8; 3]> = (0..tailles.len())
        .map(|i| if i == 0 { [0, 0, 0] } else { couleur_etiquette(i) })
        .collect();
    let mut sortie = Image::nouvelle(image.largeur, image.hauteur, 3);
    for (i, &provisoire) in provisoires.iter().enumerate() {
        let x = i % image.largeur;
        let y = i / image.largeur;
        let [r, g, b] = if provisoire == 0 {
            [0, 0, 0]
        } else {
            couleurs[compactes[equivalences.representant(provisoire)]]
        };
        sortie.modifier(x, y, 0, r);
        sortie.modifier(x, y, 1, g);
        sortie.modifier(x, y, 2, b);
    }

    (sortie, tailles)
}

/// Construit le JSON d'un étiquetage : compteur, connexité et taille de chaque classe.
fn vers_json_etiquetage_avec(
    tailles: &[usize],
    connexite: Connexite,
    cle: &str,
    nom_courbe: &str,
) -> String {
    let nb = tailles.len() - 1;
    let connexion = if connexite == Connexite::Huit { 8 } else { 4 };
    let mut json = format!("{{\n  \"valeurs\": {{\"{cle}\": {nb}, \"connexite\": {connexion}}}");
    if nb > 0 {
        let abscisses: Vec<String> = (1..=nb).map(|i| i.to_string()).collect();
        let valeurs: Vec<String> = tailles[1..].iter().map(|n| n.to_string()).collect();
        json.push_str(&format!(
            ",\n  \"courbes\": [{{\"nom\": \"{nom_courbe}\", \"style\": \"barres\", \"x\": [{}], \"valeurs\": [{}]}}]\n",
            abscisses.join(", "),
            valeurs.join(", ")
        ));
    } else {
        json.push('\n');
    }
    json.push_str("}\n");
    json
}

/// JSON de l'étiquetage des composantes (régions).
fn vers_json_etiquetage(tailles: &[usize], connexite: Connexite) -> String {
    vers_json_etiquetage_avec(tailles, connexite, "nb_composantes", "Taille des composantes")
}

/// JSON de l'étiquetage des contours.
fn vers_json_contours(tailles: &[usize]) -> String {
    vers_json_etiquetage_avec(tailles, Connexite::Huit, "nb_contours", "Taille des contours")
}

/// `traitements etiquetage <entrée.pgm> <sortie.ppm> <sortie.json> <4|8>`
pub fn executer_etiquetage(args: &[String]) -> Resultat {
    verifier_arguments(
        args,
        &["entrée.pgm", "sortie.ppm", "sortie.json", "connexité (4|8)"],
    )?;
    let image = pnm::lire(&args[0])?;
    verifier_binaire(&image)?;
    let connexite = Connexite::depuis_chaine(&args[3])?;
    let (sortie, tailles) = etiqueter_composantes(&image, connexite);

    pnm::ecrire(&args[1], &sortie)?;
    fs::write(&args[2], vers_json_etiquetage(&tailles, connexite))?;
    Ok(())
}

// ===========================================================================
// Étiquetage des contours (8-connexité)
// ===========================================================================

/// Étiquetage des contours (cours 07, diapo 23) : exactement le même algorithme
/// que pour les composantes, mais en 8-connexité — on regarde les 4 voisins
/// déjà rencontrés (haut, haut-gauche, haut-droit, gauche) au lieu de 2.
///
/// Le cours recommande des contours d'un pixel d'épaisseur (suppression des
/// non-maxima, traitement séparé) : avec des contours épais, des objets
/// voisins peuvent se retrouver fusionnés, ce qui est attendu en 8-connexité.
pub fn etiqueter_contours(image: &Image) -> (Image, Vec<usize>) {
    etiqueter_composantes(image, Connexite::Huit)
}

/// `traitements etiquetage-contours <entrée.pgm> <sortie.ppm> <sortie.json>`
pub fn executer_etiquetage_contours(args: &[String]) -> Resultat {
    verifier_arguments(args, &["entrée.pgm", "sortie.ppm", "sortie.json"])?;
    let image = pnm::lire(&args[0])?;
    verifier_binaire(&image)?;
    let (sortie, tailles) = etiqueter_contours(&image);

    pnm::ecrire(&args[1], &sortie)?;
    fs::write(&args[2], vers_json_contours(&tailles))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binaire(largeur: usize, hauteur: usize, pixels: Vec<u8>) -> Image {
        Image::depuis_pixels(largeur, hauteur, 1, pixels).unwrap()
    }

    #[test]
    fn d4_depuis_le_centre_d_un_carre_3x3() {
        // Losange du cours : 2 1 2 / 1 0 1 / 2 1 2
        let image = binaire(3, 3, vec![255; 9]);
        assert_eq!(
            carte_distance(&image, Distance::D4, 1, 1).pixels,
            vec![2, 1, 2, 1, 0, 1, 2, 1, 2]
        );
    }

    #[test]
    fn d8_depuis_le_centre_d_un_carre_3x3() {
        // Carré du cours : tous les voisins sont à 1, y compris les coins
        let image = binaire(3, 3, vec![255; 9]);
        assert_eq!(
            carte_distance(&image, Distance::D8, 1, 1).pixels,
            vec![1, 1, 1, 1, 0, 1, 1, 1, 1]
        );
    }

    #[test]
    fn le_fond_reste_noir_et_les_distances_sont_bornees() {
        let image = binaire(3, 1, vec![0, 255, 255]);
        assert_eq!(carte_distance(&image, Distance::D4, 0, 0).pixels, vec![0, 1, 2]);

        let grande = binaire(300, 1, vec![255; 300]);
        let resultat = carte_distance(&grande, Distance::D4, 0, 0);
        assert_eq!(resultat.pixels[0], 0);
        assert_eq!(resultat.pixels[255], 255);
        assert_eq!(resultat.pixels[299], 255); // 299 -> ramené à 255
    }

    #[test]
    fn refuse_une_image_non_binaire_et_une_couleur() {
        let mal = binaire(1, 1, vec![1]);
        assert!(verifier_binaire(&mal).is_err());
        let couleur = Image::nouvelle(1, 1, 3);
        assert!(verifier_binaire(&couleur).is_err());
        assert!(verifier_binaire(&binaire(1, 1, vec![255])).is_ok());
    }

    #[test]
    fn methode_inconnue_donne_un_message_clair() {
        assert!(Distance::depuis_chaine("D2").unwrap_err().contains("D4 ou D8"));
    }

    fn une(largeur: usize, hauteur: usize, positions: &[(usize, usize)]) -> Image {
        let mut pixels = vec![0; largeur * hauteur];
        for &(x, y) in positions {
            pixels[y * largeur + x] = 255;
        }
        Image::depuis_pixels(largeur, hauteur, 1, pixels).unwrap()
    }

    #[test]
    fn deux_blocs_separes_donnent_deux_composantes() {
        let image = une(7, 3, &[(0, 0), (1, 0), (0, 1), (1, 1), (5, 1), (6, 1), (5, 2), (6, 2)]);
        let (sortie, tailles) = etiqueter_composantes(&image, Connexite::Quatre);
        assert_eq!(tailles, vec![0, 4, 4]);
        assert!(sortie.est_couleur());

        let c1 = [sortie.valeur(0, 0, 0), sortie.valeur(0, 0, 1), sortie.valeur(0, 0, 2)];
        let c2 = [sortie.valeur(5, 1, 0), sortie.valeur(5, 1, 1), sortie.valeur(5, 1, 2)];
        let fond = [sortie.valeur(3, 0, 0), sortie.valeur(3, 0, 1), sortie.valeur(3, 0, 2)];
        assert_ne!(c1, [0, 0, 0]);
        assert_ne!(c2, [0, 0, 0]);
        assert_ne!(c1, c2);
        assert_eq!(fond, [0, 0, 0]);
    }

    #[test]
    fn diagonale_separee_en_4_connexite_et_fusionnee_en_8() {
        let image = une(2, 2, &[(0, 0), (1, 1)]);
        assert_eq!(etiqueter_composantes(&image, Connexite::Quatre).1, vec![0, 1, 1]);
        assert_eq!(etiqueter_composantes(&image, Connexite::Huit).1, vec![0, 2]);
    }

    #[test]
    fn equivalence_en_u_fusionne_les_deux_bras() {
        let image = une(3, 3, &[(0, 0), (0, 1), (0, 2), (1, 2), (2, 2), (2, 1), (2, 0)]);
        assert_eq!(etiqueter_composantes(&image, Connexite::Quatre).1, vec![0, 7]);
    }

    #[test]
    fn image_vide_donne_zero_composante() {
        let image = une(2, 2, &[]);
        let (sortie, tailles) = etiqueter_composantes(&image, Connexite::Huit);
        assert_eq!(tailles, vec![0]);
        assert_eq!(sortie.pixels, vec![0; 12]);
    }

    #[test]
    fn json_etiquetage_contient_nb_et_tailles() {
        let image = une(4, 1, &[(0, 0), (1, 0), (3, 0)]);
        let (_, tailles) = etiqueter_composantes(&image, Connexite::Quatre);
        let json = vers_json_etiquetage(&tailles, Connexite::Quatre);
        assert!(json.contains("\"nb_composantes\": 2"));
        assert!(json.contains("\"connexite\": 4"));
        assert!(json.contains("\"valeurs\": [2, 1]"));
    }

    // ---- Étiquetage des contours (8-connexité) ----

    #[test]
    fn contours_qui_se_touchent_en_diagonale_sont_fusionnes() {
        let image = une(4, 2, &[(0, 0), (1, 0), (2, 1), (3, 1)]);
        // En 4-connexité : deux composantes ; en 8-connexité : un seul contour.
        assert_eq!(etiqueter_composantes(&image, Connexite::Quatre).1, vec![0, 2, 2]);
        assert_eq!(etiqueter_contours(&image).1, vec![0, 4]);
    }

    #[test]
    fn une_chaine_diagonale_est_un_seul_contour() {
        let image = une(3, 3, &[(0, 0), (1, 1), (2, 2)]);
        assert_eq!(etiqueter_contours(&image).1, vec![0, 3]);
    }

    #[test]
    fn deux_contours_separes_ont_deux_couleurs_distinctes() {
        let image = une(8, 4, &[(0, 0), (1, 0), (1, 1), (6, 2), (7, 2), (7, 3)]);
        let (sortie, tailles) = etiqueter_contours(&image);

        assert_eq!(tailles, vec![0, 3, 3]);
        assert!(sortie.est_couleur());
        assert_eq!((sortie.largeur, sortie.hauteur), (8, 4));

        let c1 = [sortie.valeur(0, 0, 0), sortie.valeur(0, 0, 1), sortie.valeur(0, 0, 2)];
        let c2 = [sortie.valeur(6, 2, 0), sortie.valeur(6, 2, 1), sortie.valeur(6, 2, 2)];
        let fond = [sortie.valeur(4, 1, 0), sortie.valeur(4, 1, 1), sortie.valeur(4, 1, 2)];
        assert_ne!(c1, [0, 0, 0]);
        assert_ne!(c2, [0, 0, 0]);
        assert_ne!(c1, c2);
        assert_eq!(fond, [0, 0, 0]);
    }

    #[test]
    fn contour_ferme_donne_un_seul_contour_et_l_interieur_reste_noir() {
        let mut positions = Vec::new();
        for x in 0..5 {
            positions.push((x, 0));
            positions.push((x, 4));
        }
        for y in 1..4 {
            positions.push((0, y));
            positions.push((4, y));
        }
        let image = une(5, 5, &positions);
        let (sortie, tailles) = etiqueter_contours(&image);

        assert_eq!(tailles, vec![0, 16]); // 5 + 5 + 3 + 3 pixels de bord
        assert_eq!([sortie.valeur(2, 2, 0), sortie.valeur(2, 2, 1), sortie.valeur(2, 2, 2)], [0, 0, 0]);
        assert_ne!([sortie.valeur(0, 0, 0), sortie.valeur(0, 0, 1), sortie.valeur(0, 0, 2)], [0, 0, 0]);
    }

    #[test]
    fn deux_contours_epais_qui_se_touchent_en_diagonale_sont_unis() {
        // Deux blocs 2x2 : le cours recommande un contour d'un pixel, sinon
        // la 8-connexité fusionne les objets par leur coin (ici (1,1) et (2,2)).
        let image = une(4, 4, &[
            (0, 0), (1, 0), (0, 1), (1, 1),
            (2, 2), (3, 2), (2, 3), (3, 3),
        ]);
        assert_eq!(etiqueter_contours(&image).1, vec![0, 8]);
    }

    #[test]
    fn json_contours_contient_nb_contours_et_connexite_8() {
        let image = une(8, 4, &[(0, 0), (1, 0), (1, 1), (6, 2), (7, 2), (7, 3)]);
        let (_, tailles) = etiqueter_contours(&image);
        let json = vers_json_contours(&tailles);
        assert!(json.contains("\"nb_contours\": 2"));
        assert!(json.contains("\"connexite\": 8"));
        assert!(json.contains("\"valeurs\": [3, 3]"));
        assert!(json.contains("Taille des contours"));
    }

    #[test]
    fn nombre_d_arguments_incorrect_est_refuse() {
        let args: Vec<String> = vec!["entree.pgm".into(), "sortie.ppm".into()];
        assert!(executer_etiquetage_contours(&args).is_err());
    }

    #[test]
    fn image_non_binaire_est_refusee() {
        let dossier = std::env::temp_dir();
        let entree = dossier.join(format!("ti_contours_invalide_{}.pgm", std::process::id()));
        let image = Image::depuis_pixels(1, 1, 1, vec![1]).unwrap(); // 1 n'est pas 0 ni 255
        pnm::ecrire(&entree, &image).unwrap();

        let args: Vec<String> = vec![
            entree.to_string_lossy().into_owned(),
            dossier.join("ti_contours_sortie.ppm").to_string_lossy().into_owned(),
            dossier.join("ti_contours_sortie.json").to_string_lossy().into_owned(),
        ];
        assert!(executer_etiquetage_contours(&args).is_err());
        let _ = std::fs::remove_file(&entree);
    }
}
