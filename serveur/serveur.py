#!/usr/bin/env python3
"""
Serveur de l'interface de traitement d'image (M2 SIG).

Role : faire le lien entre le navigateur (web/) et les executables C / Rust
qui implementent les traitements. Le serveur ne fait AUCUN traitement d'image :
il recoit les images, les ecrit dans un dossier temporaire, lance l'executable
declare dans traitements.json, puis renvoie le resultat au navigateur.

Uniquement la bibliotheque standard Python (>= 3.8), aucune dependance.

Lancement (depuis la racine du depot) :
    python3 serveur/serveur.py
    python3 serveur/serveur.py --port 8080 --registre traitements.json

Voir README.md pour le lancement et DEVELOPPEMENT.md pour le contrat des executables.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import json
import logging
import math
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from contextlib import contextmanager
from http import HTTPStatus
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote, urlparse

# --------------------------------------------------------------------------
# Constantes
# --------------------------------------------------------------------------

RACINE = Path(__file__).resolve().parent.parent  # racine du depot
DOSSIER_WEB = RACINE / "web"
REGISTRE_DEFAUT = RACINE / "traitements.json"

TAILLE_MAX_REQUETE = 64 * 1024 * 1024  # 64 Mo (images encodees en base64)
DIMENSION_MAX = 10000  # largeur / hauteur maximales acceptees
TIMEOUT_DEFAUT = 30  # secondes
TAILLE_MAX_STDERR = 10000  # caracteres de stderr renvoyes au navigateur

TYPES_ENTREE = ("gris", "couleur", "binaire", "tous")
TYPES_SORTIE = ("image", "json", "image+json")
TYPES_PARAM = ("int", "float", "bool", "choix", "liste")

log = logging.getLogger("serveur")

# Options de la ligne de commande utiles a l'execution (remplies par main)
OPTIONS = {
    "dossier_temp": None,   # Path ou None (= dossier temporaire du systeme)
    "garder_temp": False,   # True = ne pas supprimer les dossiers de travail
}


class ErreurClient(Exception):
    """Erreur imputable a la requete (ou au registre) : renvoyee au navigateur."""

    def __init__(self, statut: int, message: str, details: str | None = None):
        super().__init__(message)
        self.statut = statut
        self.message = message
        self.details = details


# --------------------------------------------------------------------------
# Images PGM (P5) / PPM (P6)
# --------------------------------------------------------------------------


def lire_entete_pnm(donnees: bytes) -> dict:
    """
    Analyse l'en-tete d'une image PGM binaire (P5) ou PPM binaire (P6).

    Le contrat du projet impose : P5 ou P6, valeur max = 255 (1 octet / canal).
    Les commentaires (#...) dans l'en-tete sont acceptes.
    Leve ValueError avec un message explicite si l'image n'est pas conforme.
    """
    if len(donnees) < 2 or donnees[0:1] != b"P":
        raise ValueError("ce n'est pas une image PGM/PPM (doit commencer par 'P5' ou 'P6')")
    magique = donnees[0:2].decode("ascii", "replace")
    if magique not in ("P5", "P6"):
        raise ValueError(
            f"format {magique} non supporte : seuls P5 (PGM binaire) et P6 (PPM binaire) sont acceptes"
        )

    # Lecture des 3 entiers : largeur, hauteur, valeur max
    valeurs = []
    pos = 2
    n = len(donnees)
    while len(valeurs) < 3:
        # sauter les espaces et les commentaires
        while pos < n and (donnees[pos:pos + 1].isspace() or donnees[pos:pos + 1] == b"#"):
            if donnees[pos:pos + 1] == b"#":
                while pos < n and donnees[pos:pos + 1] not in (b"\n", b"\r"):
                    pos += 1
            else:
                pos += 1
        debut = pos
        while pos < n and donnees[pos:pos + 1].isdigit():
            pos += 1
        if debut == pos:
            raise ValueError("en-tete PGM/PPM incomplet ou invalide")
        valeurs.append(int(donnees[debut:pos]))
    # exactement UN caractere blanc entre maxval et les pixels
    if pos >= n or not donnees[pos:pos + 1].isspace():
        raise ValueError("en-tete PGM/PPM invalide (blanc attendu apres la valeur max)")
    pos += 1

    largeur, hauteur, maxval = valeurs
    if largeur <= 0 or hauteur <= 0:
        raise ValueError(f"dimensions invalides : {largeur}x{hauteur}")
    if largeur > DIMENSION_MAX or hauteur > DIMENSION_MAX:
        raise ValueError(f"image trop grande : {largeur}x{hauteur} (max {DIMENSION_MAX})")
    if maxval != 255:
        raise ValueError(f"valeur max {maxval} non supportee : doit etre 255 (8 bits)")

    canaux = 1 if magique == "P5" else 3
    attendu = largeur * hauteur * canaux
    pixels = len(donnees) - pos
    if pixels < attendu:
        raise ValueError(f"donnees tronquees : {pixels} octets de pixels, {attendu} attendus")

    return {
        "format": "pgm" if canaux == 1 else "ppm",
        "largeur": largeur,
        "hauteur": hauteur,
        "canaux": canaux,
        "debut_pixels": pos,
    }


def verifier_type_entree(donnees: bytes, entete: dict, type_attendu: str, nom: str) -> None:
    """Verifie que l'image correspond au type declare dans le registre."""
    if type_attendu == "tous":
        return
    if type_attendu == "couleur" and entete["canaux"] != 3:
        raise ErreurClient(400, f"{nom} : une image couleur (PPM/P6) est attendue, PGM recu")
    if type_attendu in ("gris", "binaire") and entete["canaux"] != 1:
        raise ErreurClient(400, f"{nom} : une image en niveaux de gris (PGM/P5) est attendue, PPM recu")
    if type_attendu == "binaire":
        debut = entete["debut_pixels"]
        fin = debut + entete["largeur"] * entete["hauteur"]
        intrus = set(donnees[debut:fin]) - {0, 255}
        if intrus:
            raise ErreurClient(
                400,
                f"{nom} : une image binaire est attendue (pixels a 0 ou 255 uniquement), "
                f"valeur {min(intrus)} trouvee",
            )


# --------------------------------------------------------------------------
# Registre des traitements (traitements.json)
# --------------------------------------------------------------------------


class Registre:
    """
    Charge et valide traitements.json.

    Le fichier est relu automatiquement des qu'il est modifie (pas besoin de
    redemarrer le serveur quand quelqu'un ajoute un traitement).
    Les entrees invalides sont ecartees et signalees dans `erreurs`.
    """

    def __init__(self, chemin: Path):
        self.chemin = chemin
        self._verrou = threading.Lock()
        self._mtime: float | None = None
        self.traitements: dict[str, dict] = {}
        self.erreurs: list[str] = []

    def actualiser(self) -> None:
        with self._verrou:
            try:
                mtime = self.chemin.stat().st_mtime
            except FileNotFoundError:
                self.traitements, self.erreurs, self._mtime = {}, [f"registre introuvable : {self.chemin}"], None
                return
            if mtime == self._mtime:
                return
            self._mtime = mtime
            self.traitements, self.erreurs = self._charger()
            log.info("registre charge : %d traitement(s), %d erreur(s)", len(self.traitements), len(self.erreurs))
            for e in self.erreurs:
                log.warning("registre : %s", e)

    def _charger(self) -> tuple[dict, list]:
        try:
            contenu = json.loads(self.chemin.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as e:
            return {}, [f"impossible de lire {self.chemin.name} : {e}"]

        liste = contenu.get("traitements") if isinstance(contenu, dict) else None
        if not isinstance(liste, list):
            return {}, ["le registre doit etre un objet de la forme {\"traitements\": [...]}"]

        traitements, erreurs = {}, []
        for i, brut in enumerate(liste):
            try:
                t = self._valider(brut)
            except ValueError as e:
                ident = brut.get("id", f"#{i}") if isinstance(brut, dict) else f"#{i}"
                erreurs.append(f"traitement '{ident}' ignore : {e}")
                continue
            if t["id"] in traitements:
                erreurs.append(f"traitement '{t['id']}' ignore : identifiant en double")
                continue
            traitements[t["id"]] = t
        return traitements, erreurs

    @staticmethod
    def _valider(brut) -> dict:
        if not isinstance(brut, dict):
            raise ValueError("doit etre un objet JSON")

        ident = brut.get("id")
        if not isinstance(ident, str) or not ident or not all(c.isalnum() or c in "-_" for c in ident):
            raise ValueError("'id' obligatoire (lettres, chiffres, '-' et '_' uniquement)")

        commande = brut.get("commande")
        if isinstance(commande, str):
            commande = [commande]
        if not isinstance(commande, list) or not commande or not all(isinstance(c, str) and c for c in commande):
            raise ValueError("'commande' obligatoire : chaine ou liste de chaines")

        entrees = brut.get("entrees", [{"nom": "Image", "type": "tous"}])
        if not isinstance(entrees, list):
            raise ValueError("'entrees' doit etre une liste")
        entrees_ok = []
        for j, e in enumerate(entrees):
            if isinstance(e, str):
                e = {"type": e}
            if not isinstance(e, dict) or e.get("type", "tous") not in TYPES_ENTREE:
                raise ValueError(f"entree {j + 1} : 'type' doit etre parmi {TYPES_ENTREE}")
            entrees_ok.append({"nom": str(e.get("nom", f"Image {j + 1}")), "type": e.get("type", "tous")})

        sortie = brut.get("sortie", "image")
        if sortie not in TYPES_SORTIE:
            raise ValueError(f"'sortie' doit etre parmi {TYPES_SORTIE}")

        params_ok = []
        noms_vus = set()
        for p in brut.get("params", []):
            params_ok.append(Registre._valider_param(p))
            if params_ok[-1]["nom"] in noms_vus:
                raise ValueError(f"parametre '{params_ok[-1]['nom']}' en double")
            noms_vus.add(params_ok[-1]["nom"])

        timeout = brut.get("timeout", TIMEOUT_DEFAUT)
        if not isinstance(timeout, (int, float)) or timeout <= 0:
            raise ValueError("'timeout' doit etre un nombre de secondes > 0")

        return {
            "id": ident,
            "nom": str(brut.get("nom", ident)),
            "chapitre": str(brut.get("chapitre", "Divers")),
            "description": str(brut.get("description", "")),
            "auteur": str(brut.get("auteur", "")),
            "langage": str(brut.get("langage", "")),
            "commande": commande,
            "entrees": entrees_ok,
            "sortie": sortie,
            "params": params_ok,
            "timeout": timeout,
        }

    @staticmethod
    def _valider_param(p) -> dict:
        if not isinstance(p, dict) or not isinstance(p.get("nom"), str) or not p["nom"]:
            raise ValueError("chaque parametre doit avoir un 'nom'")
        nom, type_ = p["nom"], p.get("type")
        if type_ not in TYPES_PARAM:
            raise ValueError(f"parametre '{nom}' : 'type' doit etre parmi {TYPES_PARAM}")
        if type_ == "choix":
            options = p.get("options")
            if not isinstance(options, list) or not options or not all(isinstance(o, str) for o in options):
                raise ValueError(f"parametre '{nom}' : 'options' (liste de chaines) obligatoire pour le type choix")
        if "defaut" not in p:
            raise ValueError(f"parametre '{nom}' : 'defaut' obligatoire")
        param = dict(p)
        param.setdefault("label", nom)
        if not isinstance(param.get("aide", ""), str):
            raise ValueError(f"parametre '{nom}' : 'aide' doit etre un texte")
        # la valeur par defaut doit elle-meme etre valide
        try:
            param["defaut"] = convertir_param(param, p["defaut"])[0]
        except ErreurClient as e:
            raise ValueError(f"valeur par defaut invalide : {e.message}")
        return param

    def commande_disponible(self, t: dict) -> tuple[bool, str]:
        """Indique si l'executable existe (pour griser le traitement dans l'interface)."""
        chemin = resoudre_executable(t["commande"][0])
        if chemin is None:
            return False, f"executable introuvable : {t['commande'][0]} (pas encore compile ?)"
        return True, ""


def resoudre_executable(nom: str) -> str | None:
    """
    Un chemin contenant '/' est relatif a la racine du depot (ex : c/bin/sobel).
    Sinon, on cherche dans le PATH (ex : python3, cargo).
    """
    if "/" in nom or os.sep in nom:
        chemin = Path(nom) if Path(nom).is_absolute() else RACINE / nom
        if chemin.is_file() and os.access(chemin, os.X_OK):
            return str(chemin)
        # Windows : essayer avec .exe
        if (chemin.with_suffix(".exe")).is_file():
            return str(chemin.with_suffix(".exe"))
        return None
    return shutil.which(nom)


# --------------------------------------------------------------------------
# Parametres
# --------------------------------------------------------------------------


def convertir_param(param: dict, valeur) -> tuple[object, str]:
    """
    Valide une valeur de parametre et la convertit en argument de ligne de commande.
    Retourne (valeur normalisee, argument texte).
    """
    nom, type_ = param["nom"], param["type"]

    def borner(v):
        if "min" in param and v < param["min"]:
            raise ErreurClient(400, f"parametre '{nom}' : {v} < minimum {param['min']}")
        if "max" in param and v > param["max"]:
            raise ErreurClient(400, f"parametre '{nom}' : {v} > maximum {param['max']}")
        return v

    if type_ == "int":
        if isinstance(valeur, bool):
            raise ErreurClient(400, f"parametre '{nom}' : entier attendu")
        try:
            v = int(valeur) if not isinstance(valeur, float) or valeur.is_integer() else None
        except (TypeError, ValueError):
            v = None
        if v is None:
            raise ErreurClient(400, f"parametre '{nom}' : entier attendu, recu {valeur!r}")
        v = borner(v)
        return v, str(v)

    if type_ == "float":
        if isinstance(valeur, bool):
            raise ErreurClient(400, f"parametre '{nom}' : nombre attendu")
        try:
            v = float(valeur)
        except (TypeError, ValueError):
            raise ErreurClient(400, f"parametre '{nom}' : nombre attendu, recu {valeur!r}")
        if not math.isfinite(v):
            raise ErreurClient(400, f"parametre '{nom}' : nombre fini attendu")
        v = borner(v)
        return v, repr(v)

    if type_ == "bool":
        if isinstance(valeur, bool):
            v = valeur
        elif valeur in (0, 1, "0", "1", "true", "false"):
            v = valeur in (1, "1", "true")
        else:
            raise ErreurClient(400, f"parametre '{nom}' : booleen attendu, recu {valeur!r}")
        return v, "1" if v else "0"

    if type_ == "choix":
        if valeur not in param["options"]:
            raise ErreurClient(400, f"parametre '{nom}' : {valeur!r} n'est pas parmi {param['options']}")
        return valeur, valeur

    if type_ == "liste":
        if isinstance(valeur, str):
            morceaux = [m for m in valeur.replace(";", ",").replace(" ", ",").split(",") if m]
        elif isinstance(valeur, list):
            morceaux = valeur
        else:
            raise ErreurClient(400, f"parametre '{nom}' : liste de nombres attendue")
        try:
            nombres = [float(m) for m in morceaux]
        except (TypeError, ValueError):
            raise ErreurClient(400, f"parametre '{nom}' : liste de nombres attendue, recu {valeur!r}")
        if not nombres or not all(math.isfinite(x) for x in nombres):
            raise ErreurClient(400, f"parametre '{nom}' : liste vide ou invalide")
        if "taille" in param and len(nombres) != param["taille"]:
            raise ErreurClient(400, f"parametre '{nom}' : {param['taille']} valeurs attendues, {len(nombres)} recues")
        nombres = [int(x) if x.is_integer() else x for x in nombres]
        return nombres, ",".join(str(x) for x in nombres)

    raise ErreurClient(400, f"parametre '{nom}' : type inconnu")


# --------------------------------------------------------------------------
# Execution d'un traitement
# --------------------------------------------------------------------------


def decoder_image_base64(texte, nom: str) -> bytes:
    if not isinstance(texte, str) or not texte:
        raise ErreurClient(400, f"{nom} : image manquante (chaine base64 attendue)")
    if texte.startswith("data:"):  # tolere les data-URL
        texte = texte.split(",", 1)[-1]
    try:
        return base64.b64decode(texte, validate=True)
    except (binascii.Error, ValueError):
        raise ErreurClient(400, f"{nom} : base64 invalide")


@contextmanager
def dossier_de_travail(ident: str):
    """
    Cree le dossier ou sont ecrits les fichiers d'entree et de sortie d'UNE requete,
    puis le supprime a la fin (succes ou erreur).

    Emplacement :
      - par defaut, le dossier temporaire du systeme (tempfile.gettempdir()) :
          Linux   : /tmp/ti_<id>_XXXXXXXX/
          macOS   : /var/folders/.../T/ti_<id>_XXXXXXXX/
          Windows : C:\\Users\\<nom>\\AppData\\Local\\Temp\\ti_<id>_XXXXXXXX\\
      - avec --dossier-temp D : sous D (utile si le chemin du systeme pose probleme,
        par exemple un nom d'utilisateur Windows avec des caracteres speciaux).
    Le chemin est toujours ABSOLU, et chaque requete a son propre dossier (nom aleatoire) :
    deux requetes simultanees ne se melangent jamais.

    Avec --garder-temp, le dossier n'est pas supprime (pour inspecter les fichiers).
    """
    parent = OPTIONS["dossier_temp"]
    if parent is not None:
        parent.mkdir(parents=True, exist_ok=True)
    dossier = Path(tempfile.mkdtemp(prefix=f"ti_{ident}_", dir=parent)).resolve()
    try:
        yield dossier
    finally:
        if OPTIONS["garder_temp"]:
            log.info("[%s] fichiers conserves dans %s", ident, dossier)
        else:
            # ignore_errors : sous Windows, un antivirus peut verrouiller un fichier quelques
            # instants ; on ne transforme pas un traitement reussi en erreur pour autant.
            shutil.rmtree(dossier, ignore_errors=True)


def executer(t: dict, requete: dict) -> dict:
    """Execute le traitement `t` avec les images et parametres de `requete`."""
    if not isinstance(requete, dict):
        raise ErreurClient(400, "le corps de la requete doit etre un objet JSON")

    # 1. Images d'entree
    images = requete.get("images", [])
    if not isinstance(images, list):
        raise ErreurClient(400, "'images' doit etre une liste")
    if len(images) != len(t["entrees"]):
        raise ErreurClient(400, f"{len(t['entrees'])} image(s) attendue(s), {len(images)} recue(s)")

    donnees_entrees = []
    for img_b64, spec in zip(images, t["entrees"]):
        donnees = decoder_image_base64(img_b64, spec["nom"])
        try:
            entete = lire_entete_pnm(donnees)
        except ValueError as e:
            raise ErreurClient(400, f"{spec['nom']} : {e}")
        verifier_type_entree(donnees, entete, spec["type"], spec["nom"])
        donnees_entrees.append((donnees, entete))

    # 2. Parametres (ordre = ordre du registre, valeur par defaut si absent)
    params_recus = requete.get("params", {})
    if not isinstance(params_recus, dict):
        raise ErreurClient(400, "'params' doit etre un objet {nom: valeur}")
    inconnus = set(params_recus) - {p["nom"] for p in t["params"]}
    if inconnus:
        raise ErreurClient(400, f"parametre(s) inconnu(s) : {', '.join(sorted(inconnus))}")
    params_utilises, args_params = {}, []
    for p in t["params"]:
        valeur, arg = convertir_param(p, params_recus.get(p["nom"], p["defaut"]))
        params_utilises[p["nom"]] = valeur
        args_params.append(arg)

    # 3. Executable
    executable = resoudre_executable(t["commande"][0])
    if executable is None:
        raise ErreurClient(
            503, f"executable introuvable pour '{t['id']}' : {t['commande'][0]}",
            "Le traitement n'est pas encore compile (voir « Démarrage rapide » dans le README).",
        )

    # 4. Dossier de travail isole par requete (voir dossier_de_travail)
    with dossier_de_travail(t["id"]) as dossier:
        chemins_entrees = []
        for i, (donnees, entete) in enumerate(donnees_entrees, start=1):
            chemin = dossier / f"entree{i}.{entete['format']}"
            chemin.write_bytes(donnees)
            chemins_entrees.append(str(chemin))

        chemin_image = dossier / "sortie.pnm"
        chemin_json = dossier / "sortie.json"
        chemins_sorties = {
            "image": [str(chemin_image)],
            "json": [str(chemin_json)],
            "image+json": [str(chemin_image), str(chemin_json)],
        }[t["sortie"]]

        # Liste d'arguments (jamais de shell => pas d'injection possible)
        cmd = [executable] + t["commande"][1:] + chemins_entrees + chemins_sorties + args_params
        log.info("[%s] %s", t["id"], " ".join(cmd))

        debut = time.perf_counter()
        try:
            proc = subprocess.run(
                cmd, cwd=RACINE, capture_output=True, timeout=t["timeout"], stdin=subprocess.DEVNULL,
            )
        except subprocess.TimeoutExpired:
            raise ErreurClient(504, f"'{t['id']}' a depasse le temps limite ({t['timeout']} s)")
        except OSError as e:
            raise ErreurClient(500, f"impossible de lancer '{t['commande'][0]}' : {e}")
        duree_ms = round((time.perf_counter() - debut) * 1000, 1)

        stdout = proc.stdout.decode("utf-8", "replace")[-TAILLE_MAX_STDERR:]
        stderr = proc.stderr.decode("utf-8", "replace")[-TAILLE_MAX_STDERR:]

        if proc.returncode != 0:
            raise ErreurClient(
                500, f"'{t['id']}' a echoue (code de retour {proc.returncode})", stderr or stdout or None,
            )

        reponse = {
            "traitement": t["id"],
            "duree_ms": duree_ms,
            "params": params_utilises,
            "commande": " ".join([Path(cmd[0]).name] + cmd[1:]).replace(str(dossier) + os.sep, ""),
            "stdout": stdout,
            "stderr": stderr,
        }

        # 5. Lecture et validation des sorties
        if t["sortie"] in ("image", "image+json"):
            if not chemin_image.is_file():
                raise ErreurClient(500, f"'{t['id']}' n'a pas ecrit d'image de sortie", stderr or None)
            donnees = chemin_image.read_bytes()
            try:
                entete = lire_entete_pnm(donnees)
            except ValueError as e:
                raise ErreurClient(500, f"'{t['id']}' a produit une image invalide : {e}", stderr or None)
            reponse["image"] = {
                "format": entete["format"],
                "largeur": entete["largeur"],
                "hauteur": entete["hauteur"],
                "donnees": base64.b64encode(donnees).decode("ascii"),
            }

        if t["sortie"] in ("json", "image+json"):
            if not chemin_json.is_file():
                raise ErreurClient(500, f"'{t['id']}' n'a pas ecrit de fichier JSON de sortie", stderr or None)
            try:
                reponse["json"] = json.loads(chemin_json.read_text(encoding="utf-8"))
            except (UnicodeDecodeError, json.JSONDecodeError) as e:
                raise ErreurClient(500, f"'{t['id']}' a produit un JSON invalide : {e}", stderr or None)

        return reponse


# --------------------------------------------------------------------------
# Serveur HTTP
# --------------------------------------------------------------------------


class Gestionnaire(SimpleHTTPRequestHandler):
    """
    Routes :
      GET  /api/traitements        -> liste des traitements (registre)
      POST /api/traitements/<id>   -> execute un traitement
      GET  /<fichier>              -> fichiers statiques de web/ (interface)
    """

    registre: Registre  # injecte au demarrage
    server_version = "TraitementImage/1.0"

    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(DOSSIER_WEB), **kwargs)

    # -- utilitaires ---------------------------------------------------------

    def envoyer_json(self, statut: int, objet) -> None:
        corps = json.dumps(objet, ensure_ascii=False).encode("utf-8")
        self.send_response(statut)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(corps)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(corps)

    def envoyer_erreur(self, e: ErreurClient) -> None:
        self.envoyer_json(e.statut, {"erreur": e.message, "details": e.details})

    def log_message(self, format, *args):  # journal plus sobre
        log.debug("%s - %s", self.address_string(), format % args)

    def end_headers(self):
        # l'interface est en developpement : pas de cache navigateur
        if not self.path.startswith("/api/"):
            self.send_header("Cache-Control", "no-cache")
        super().end_headers()

    # -- routes --------------------------------------------------------------

    def do_GET(self):
        chemin = urlparse(self.path).path
        if chemin in ("/api/traitements", "/api/traitements/"):
            return self.lister_traitements()
        if chemin.startswith("/api/"):
            return self.envoyer_json(404, {"erreur": f"route inconnue : {chemin}"})
        if chemin == "/" and not (DOSSIER_WEB / "index.html").is_file():
            return self.envoyer_json(200, {
                "message": "Serveur actif. L'interface (web/index.html) n'existe pas encore.",
                "api": ["GET /api/traitements", "POST /api/traitements/<id>"],
            })
        return super().do_GET()  # fichiers statiques (protege contre '../')

    def do_POST(self):
        chemin = urlparse(self.path).path
        prefixe = "/api/traitements/"
        if not chemin.startswith(prefixe) or chemin == prefixe:
            return self.envoyer_json(404, {"erreur": f"route inconnue : {chemin}"})
        ident = unquote(chemin[len(prefixe):]).strip("/")

        try:
            self.registre.actualiser()
            t = self.registre.traitements.get(ident)
            if t is None:
                raise ErreurClient(404, f"traitement inconnu : '{ident}'")
            requete = self.lire_corps_json()
            reponse = executer(t, requete)
        except ErreurClient as e:
            log.warning("[%s] %d %s", ident, e.statut, e.message)
            return self.envoyer_erreur(e)
        except Exception:  # bug du serveur : on ne plante pas, on le signale
            log.exception("[%s] erreur interne", ident)
            return self.envoyer_json(500, {"erreur": "erreur interne du serveur (voir le terminal du serveur)"})

        log.info("[%s] OK en %.1f ms", ident, reponse["duree_ms"])
        self.envoyer_json(200, reponse)

    def lire_corps_json(self):
        try:
            taille = int(self.headers.get("Content-Length", ""))
        except ValueError:
            raise ErreurClient(411, "en-tete Content-Length manquant")
        if taille > TAILLE_MAX_REQUETE:
            raise ErreurClient(413, f"requete trop volumineuse (max {TAILLE_MAX_REQUETE // 2**20} Mo)")
        corps = self.rfile.read(taille)
        try:
            return json.loads(corps)
        except (UnicodeDecodeError, json.JSONDecodeError) as e:
            raise ErreurClient(400, f"corps JSON invalide : {e}")

    def lister_traitements(self):
        self.registre.actualiser()
        liste = []
        for t in self.registre.traitements.values():
            dispo, raison = self.registre.commande_disponible(t)
            publique = {k: v for k, v in t.items() if k != "commande"}
            publique.update(disponible=dispo, raison=raison)
            liste.append(publique)
        self.envoyer_json(200, {"traitements": liste, "erreurs_registre": self.registre.erreurs})


def main():
    parser = argparse.ArgumentParser(description="Serveur de l'interface de traitement d'image")
    parser.add_argument("--hote", default="127.0.0.1",
                        help="adresse d'ecoute (defaut : 127.0.0.1, local uniquement)")
    parser.add_argument("--port", type=int, default=8000, help="port d'ecoute (defaut : 8000)")
    parser.add_argument("--registre", type=Path, default=REGISTRE_DEFAUT,
                        help="chemin du registre des traitements (defaut : traitements.json)")
    parser.add_argument("--dossier-temp", type=Path, default=None,
                        help="dossier ou creer les dossiers de travail (defaut : dossier temporaire du systeme)")
    parser.add_argument("--garder-temp", action="store_true",
                        help="ne pas supprimer les dossiers de travail (pour inspecter les fichiers echanges)")
    parser.add_argument("-v", "--verbeux", action="store_true", help="journal detaille (chaque requete HTTP)")
    args = parser.parse_args()

    OPTIONS["dossier_temp"] = args.dossier_temp.resolve() if args.dossier_temp else None
    OPTIONS["garder_temp"] = args.garder_temp

    logging.basicConfig(
        level=logging.DEBUG if args.verbeux else logging.INFO,
        format="%(asctime)s %(levelname)-7s %(message)s",
        datefmt="%H:%M:%S",
    )

    Gestionnaire.registre = Registre(args.registre.resolve())
    Gestionnaire.registre.actualiser()

    serveur = ThreadingHTTPServer((args.hote, args.port), Gestionnaire)
    log.info("interface : http://%s:%d/", args.hote, args.port)
    log.info("registre  : %s", args.registre.resolve())
    log.info("fichiers  : %s%s", OPTIONS["dossier_temp"] or tempfile.gettempdir(),
             " (conserves)" if OPTIONS["garder_temp"] else "")
    try:
        serveur.serve_forever()
    except KeyboardInterrupt:
        log.info("arret du serveur")
    finally:
        serveur.server_close()


if __name__ == "__main__":
    sys.exit(main())
