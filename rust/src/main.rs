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

mod binaire;
mod croissance;
mod division_fusion;
mod egalisation;
mod freeman;
mod gris;
mod histogramme;
mod kmeans;
mod morphologie;
mod multiseuillage;
mod otsu;
mod pnm;
mod seuillage;
mod seuillage_local;

use std::error::Error;
use std::process::ExitCode;

/// Résultat d'un traitement : `Ok(())`, ou une erreur dont le message part sur stderr.
pub type Resultat = Result<(), Box<dyn Error>>;

/// Sous-commande -> (fonction, description des arguments).
const TRAITEMENTS: &[(&str, fn(&[String]) -> Resultat, &str)] = &[
    ("histogramme", histogramme::executer, "<entrée.pgm|ppm> <sortie.json>"),
    ("egalisation", egalisation::executer, "<entrée.pgm> <sortie.pgm> <globale|locale> <fenêtre>"),
    ("distances", binaire::executer_distances, "<entrée.pgm> <sortie.pgm> <D4|D8> <germe x> <germe y>"),
    ("etiquetage", binaire::executer_etiquetage, "<entrée.pgm> <sortie.ppm> <sortie.json> <4|8>"),
    ("etiquetage-contours", binaire::executer_etiquetage_contours, "<entrée.pgm> <sortie.ppm> <sortie.json>"),
    ("freeman", freeman::executer, "<entrée.pgm> <sortie.pgm> <sortie.json> <4|8>"),
    ("morphologie", morphologie::executer, "<entrée.pgm> <sortie.pgm> <erosion|dilatation|ouverture|fermeture|gradient_interne|gradient_externe|gradient_morphologique> <4|8>"),
    ("seuillage", seuillage::executer, "<entrée> <sortie.pnm> <seuil>"),
    ("multiseuillage", multiseuillage::executer, "<entrée> <sortie.pnm> <seuils>"),
    ("otsu", otsu::executer, "<entrée> <sortie.pnm> <sortie.json>"),
    ("seuillage-local", seuillage_local::executer, "<entrée> <sortie.pnm> <fenêtre> <constante> <poids_variance>"),
    ("kmeans", kmeans::executer, "<entrée> <sortie.pnm> <sortie.json> <k> <iterations>"),
    ("division-fusion", division_fusion::executer, "<entrée> <sortie.pnm> <ecart_max> <taille_min>"),
    ("croissance", croissance::executer, "<entrée> <sortie.pnm> <sortie.json> <germes> <tolérance> <connexité>"),
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
