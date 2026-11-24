//! Point d'entrée de l'exécutable Rust du TP.
//!
//! Un seul programme, une sous-commande par traitement :
//! ```text
//! traitements <sous-commande> <entrées...> <sorties...> <paramètres...>
//! traitements histogramme entree.pgm sortie.json
//! traitements egalisation entree.pgm sortie.pgm globale 7
//! ```
//! L'ordre des arguments suit le contrat de DEVELOPPEMENT.md (« Contrat d'un exécutable »).
//!
//! Ajouter un traitement :
//!   1. créer `src/mon_traitement.rs` avec une fonction `pub fn executer(args: &[String]) -> Resultat`
//!   2. le déclarer ci-dessous (`mod mon_traitement;`) et l'ajouter dans `TRAITEMENTS`
//!   3. le déclarer dans `traitements.json` avec
//!      "commande": ["rust/target/release/traitements", "<sous-commande>"]

mod contours;
mod convolution;
mod egalisation;
mod filtrage;
mod histogramme;
mod interpolation;
mod pnm;

use std::error::Error;
use std::process::ExitCode;

/// Résultat d'un traitement : `Ok(())`, ou une erreur dont le message part sur stderr.
pub type Resultat = Result<(), Box<dyn Error>>;

/// Sous-commande -> (fonction, description des arguments).
const TRAITEMENTS: &[(&str, fn(&[String]) -> Resultat, &str)] = &[
    ("histogramme", histogramme::executer, "<entrée.pgm|ppm> <sortie.json>"),
    ("egalisation", egalisation::executer, "<entrée.pgm> <sortie.pgm> <globale|locale> <fenêtre>"),
    // Lot « Filtrage spatial » (Naomi)
    ("interpolation", interpolation::executer, "<entrée> <sortie> <plus-proche-voisin|bilineaire|bicubique> <facteur>"),
    ("convolution", convolution::executer, "<entrée> <sortie> <noyau a,b,c,...> <zero|partielle|miroir> <normaliser 1|0>"),
    ("moyenneur", filtrage::executer_moyenneur, "<entrée> <sortie> <taille> <zero|partielle|miroir>"),
    ("gaussien", filtrage::executer_gaussien, "<entrée> <sortie> <sigma> <zero|partielle|miroir>"),
    ("passe-haut", filtrage::executer_passe_haut, "<entrée> <sortie> <4|8> <zero|partielle|miroir>"),
    ("median", filtrage::executer_median, "<entrée> <sortie> <taille>"),
    ("minimum", filtrage::executer_min, "<entrée> <sortie> <taille>"),
    ("maximum", filtrage::executer_max, "<entrée> <sortie> <taille>"),
    ("poivre-et-sel", filtrage::executer_poivre_et_sel, "<entrée> <sortie> <proportion 0..1> <graine>"),
    // Lot « Contours » (Cinthia)
    ("derivee", contours::executer_derivee, "<entrée> <sortie> <x|y>"),
    ("gradient", contours::executer_gradient, "<entrée> <sortie> <roberts|prewitt|sobel> <norme|gx|gy|direction|seuillage> <seuil 0..255>"),
    ("laplacien", contours::executer_laplacien, "<entrée> <sortie> <4|8> <laplacien|passages-zero> <seuil 0..255>"),
    ("hough", contours::executer_hough, "<entrée> <sortie.pgm> <sortie.json> <seuil 1..255> <nombre de droites>"),
    ("amincissement", contours::executer_amincissement, "<entrée> <sortie> <sobel|prewitt>"),
    ("hysteresis", contours::executer_hysteresis, "<entrée> <sortie> <seuil bas> <seuil haut>"),
    ("fermeture-contours", contours::executer_fermeture, "<contours> <force> <sortie> <seuil 0..255> <longueur max>"),
];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    let Some(sous_commande) = args.get(1) else {
        eprintln!("{}", aide());
        return ExitCode::from(1);
    };
    if matches!(sous_commande.as_str(), "aide" | "-h" | "--help") {
        println!("{}", aide());
        return ExitCode::SUCCESS;
    }

    let Some((_, executer, _)) = TRAITEMENTS.iter().find(|(nom, _, _)| nom == sous_commande) else {
        eprintln!("traitement inconnu : « {sous_commande} »\n\n{}", aide());
        return ExitCode::from(1);
    };

    match executer(&args[2..]) {
        Ok(()) => ExitCode::SUCCESS,
        Err(erreur) => {
            // Ce message est affiché tel quel dans l'interface (via le serveur)
            eprintln!("{sous_commande} : {erreur}");
            ExitCode::from(1)
        }
    }
}

fn aide() -> String {
    let mut texte = String::from("usage : traitements <sous-commande> <arguments...>\n\nsous-commandes :\n");
    for (nom, _, usage) in TRAITEMENTS {
        texte.push_str(&format!("  {nom} {usage}\n"));
    }
    texte
}

/// Vérifie le nombre d'arguments reçus et produit un message clair sinon.
/// `noms` décrit les arguments attendus, dans l'ordre (ex. `&["entrée", "sortie"]`).
pub fn verifier_arguments(args: &[String], noms: &[&str]) -> Resultat {
    if args.len() != noms.len() {
        return Err(format!(
            "{} argument(s) attendu(s) ({}), {} reçu(s)",
            noms.len(),
            noms.join(", "),
            args.len()
        )
        .into());
    }
    Ok(())
}
