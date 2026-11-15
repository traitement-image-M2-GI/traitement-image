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

mod egalisation;
mod histogramme;
mod pnm;

use std::error::Error;
use std::process::ExitCode;

/// Résultat d'un traitement : `Ok(())`, ou une erreur dont le message part sur stderr.
pub type Resultat = Result<(), Box<dyn Error>>;

/// Sous-commande -> (fonction, description des arguments).
const TRAITEMENTS: &[(&str, fn(&[String]) -> Resultat, &str)] = &[
    ("histogramme", histogramme::executer, "<entrée.pgm|ppm> <sortie.json>"),
    ("egalisation", egalisation::executer, "<entrée.pgm> <sortie.pgm> <globale|locale> <fenêtre>"),
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
