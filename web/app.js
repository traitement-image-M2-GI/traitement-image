// Interface du TP de traitement d'image.
// Rôle : charger les images, construire les menus à partir du registre (GET /api/traitements),
// envoyer images + paramètres au serveur (POST /api/traitements/<id>) et afficher le résultat.
// Aucun traitement d'image n'est fait ici, sauf les conversions de format nécessaires à l'envoi
// (couleur -> gris quand le traitement attend du gris). Voir DEVELOPPEMENT.md, section « Interface web ».

import {
  chargerFichier, decoderPNM, encoderPNM, versImageData, versGris, versCouleur,
  estGris, estBinaire, versBase64, depuisBase64,
} from "./pnm.js";
import { afficherJSON } from "./graphique.js";

const $ = (sel, racine = document) => racine.querySelector(sel);
const $$ = (sel, racine = document) => [...racine.querySelectorAll(sel)];

const LETTRES = ["A", "B"];
const LIBELLE_TYPE = { gris: "niveaux de gris", couleur: "couleur", binaire: "binaire", tous: "gris ou couleur" };
const LIBELLE_SORTIE = { image: "Image", json: "Données", "image+json": "Image + données" };
const DIMENSION_MAX = 10000;

const etat = {
  traitements: [],
  signatureRegistre: "",
  chapitre: "Tous",
  recherche: "",
  choisi: null,            // id du traitement sélectionné
  valeursParams: {},       // id -> { nom: valeur } (mémorisé quand on change de traitement)
  entrees: [null, null],   // { img, nom, taille }
  resultat: null,          // { img, json, traitement }
  enCours: false,
  disposition: "grille",
};

const cartesEntree = $$(".carte-entree");
const carteResultat = $("#carte-resultat");

// =====================================================================
// Bandeaux d'information (erreurs, avertissements)
// =====================================================================

function afficherBandeau(cle, type, html) {
  let b = $(`.bandeau[data-cle="${cle}"]`);
  if (!b) {
    b = document.createElement("div");
    b.className = "bandeau";
    b.dataset.cle = cle;
    b.innerHTML = `<div></div><button type="button" aria-label="Fermer">×</button>`;
    b.querySelector("button").addEventListener("click", () => b.remove());
    $("#bandeaux").appendChild(b);
  }
  b.classList.toggle("erreur", type === "erreur");
  b.querySelector("div").innerHTML = html;
}
const retirerBandeau = (cle) => $(`.bandeau[data-cle="${cle}"]`)?.remove();

function echapper(t) {
  return String(t ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
}

// =====================================================================
// Registre des traitements
// =====================================================================

async function chargerRegistre() {
  const pastille = $("#etat-serveur");
  if (location.protocol === "file:") {
    pastille.dataset.etat = "erreur";
    pastille.lastElementChild.textContent = "Ouvert sans serveur";
    afficherBandeau("serveur", "erreur",
      "La page est ouverte directement depuis le disque. Démarrez le serveur du projet " +
      "(voir « Démarrage rapide » dans le README), puis ouvrez l'adresse qu'il indique.");
    return;
  }
  let donnees;
  try {
    const rep = await fetch("/api/traitements", { cache: "no-store" });
    if (!rep.ok) throw new Error(`HTTP ${rep.status}`);
    donnees = await rep.json();
  } catch (e) {
    pastille.dataset.etat = "erreur";
    pastille.lastElementChild.textContent = "Serveur injoignable";
    afficherBandeau("serveur", "erreur",
      "Impossible de joindre le serveur. Vérifiez qu'il est toujours démarré (voir README), puis rechargez la page.");
    majBoutonAppliquer();
    return;
  }
  retirerBandeau("serveur");

  const signature = JSON.stringify(donnees);
  if (signature === etat.signatureRegistre) return; // rien n'a changé
  etat.signatureRegistre = signature;
  etat.traitements = donnees.traitements || [];

  const n = etat.traitements.length;
  const dispo = etat.traitements.filter((t) => t.disponible).length;
  pastille.dataset.etat = "ok";
  pastille.lastElementChild.textContent = `Serveur · ${n} traitement${n > 1 ? "s" : ""}`;
  $("#compte-traitements").textContent = n
    ? `${n} traitement${n > 1 ? "s" : ""} déclaré${n > 1 ? "s" : ""}, ${dispo} compilé${dispo > 1 ? "s" : ""}.`
    : "Aucun traitement déclaré pour l'instant.";

  const erreurs = donnees.erreurs_registre || [];
  if (erreurs.length) {
    afficherBandeau("registre", "alerte",
      `<strong>traitements.json</strong> : ${erreurs.length} entrée(s) ignorée(s)` +
      `<ul>${erreurs.map((e) => `<li>${echapper(e)}</li>`).join("")}</ul>`);
  } else retirerBandeau("registre");

  if (etat.choisi && !traitementChoisi()) etat.choisi = null;
  if (!etat.traitements.some((t) => t.chapitre === etat.chapitre)) etat.chapitre = "Tous";
  afficherChapitres();
  afficherListe();
  afficherParams();
  majEmplacements();
}

const traitementChoisi = () => etat.traitements.find((t) => t.id === etat.choisi) || null;

function afficherChapitres() {
  const conteneur = $("#chapitres");
  const compte = new Map([["Tous", etat.traitements.length]]);
  for (const t of etat.traitements) compte.set(t.chapitre, (compte.get(t.chapitre) || 0) + 1);
  conteneur.innerHTML = "";
  for (const [chapitre, n] of compte) {
    const b = document.createElement("button");
    b.type = "button";
    b.className = chapitre === etat.chapitre ? "actif" : "";
    b.innerHTML = `${echapper(chapitre)}<span>${n}</span>`;
    b.addEventListener("click", () => {
      etat.chapitre = chapitre;
      afficherChapitres();
      afficherListe();
    });
    conteneur.appendChild(b);
  }
  conteneur.querySelector(".actif")?.scrollIntoView({ block: "nearest", inline: "nearest" });
}

function afficherListe() {
  const liste = $("#liste-traitements");
  const q = etat.recherche.trim().toLowerCase().normalize("NFD").replace(/\p{Diacritic}/gu, "");
  const visibles = etat.traitements.filter((t) => {
    if (etat.chapitre !== "Tous" && t.chapitre !== etat.chapitre) return false;
    if (!q) return true;
    const texte = `${t.id} ${t.nom} ${t.description} ${t.chapitre} ${t.auteur} ${t.langage}`
      .toLowerCase().normalize("NFD").replace(/\p{Diacritic}/gu, "");
    return texte.includes(q);
  });

  liste.innerHTML = "";
  if (!etat.traitements.length) {
    liste.innerHTML = `<div class="liste-vide">Le registre est vide.<br>Déclarez vos traitements dans <code>traitements.json</code> (voir DEVELOPPEMENT.md).</div>`;
    return;
  }
  if (!visibles.length) {
    liste.innerHTML = `<div class="liste-vide">Aucun traitement ne correspond.</div>`;
    return;
  }

  for (const t of visibles) {
    const b = document.createElement("button");
    b.type = "button";
    b.setAttribute("role", "option");
    b.setAttribute("aria-selected", t.id === etat.choisi);
    b.className = `traitement${t.id === etat.choisi ? " actif" : ""}${t.disponible ? "" : " indisponible"}`;
    const nbE = t.entrees.length;
    b.innerHTML = `
      <span class="traitement-haut">
        <span class="traitement-nom">${echapper(t.nom)}</span>
        <span class="puce${nbE === 2 ? " puce-forte" : ""}">${nbE} entrée${nbE > 1 ? "s" : ""}</span>
        ${t.sortie !== "image" ? `<span class="puce">${LIBELLE_SORTIE[t.sortie]}</span>` : ""}
        ${t.langage ? `<span class="puce">${echapper(t.langage)}</span>` : ""}
        ${t.disponible ? "" : `<span class="puce puce-alerte" title="${echapper(t.raison)}">non compilé</span>`}
      </span>
      ${t.description ? `<span class="traitement-desc">${echapper(t.description)}</span>` : ""}`;
    b.addEventListener("click", () => choisir(t.id));
    b.addEventListener("dblclick", () => appliquer());
    liste.appendChild(b);
  }
}

function choisir(id) {
  etat.choisi = id;
  afficherListe();
  afficherParams();
  majEmplacements();
}

// =====================================================================
// Formulaire des paramètres (généré à partir du registre)
// =====================================================================

function valeursCourantes(t) {
  if (!etat.valeursParams[t.id]) {
    etat.valeursParams[t.id] = Object.fromEntries(t.params.map((p) => [p.nom, structuredClone(p.defaut)]));
  }
  // un paramètre ajouté au registre depuis : on lui donne sa valeur par défaut
  for (const p of t.params) if (!(p.nom in etat.valeursParams[t.id])) etat.valeursParams[t.id][p.nom] = structuredClone(p.defaut);
  return etat.valeursParams[t.id];
}

function afficherParams() {
  const t = traitementChoisi();
  $("#section-params").hidden = !t;
  if (!t) return;

  $("#params-titre").textContent = t.nom;
  $("#params-description").textContent = t.description || "";
  $("#params-description").hidden = !t.description;
  const puces = [t.chapitre, t.langage, t.auteur && `par ${t.auteur}`, `id : ${t.id}`].filter(Boolean);
  $("#params-puces").innerHTML = puces.map((p) => `<span class="puce">${echapper(p)}</span>`).join("");

  const valeurs = valeursCourantes(t);
  const form = $("#form-params");
  form.innerHTML = "";
  for (const p of t.params) form.appendChild(creerControle(p, valeurs));
  majBoutonAppliquer();
}

// ---------------------------------------------------------------------
// Contrôles du formulaire : UNE fabrique par type de paramètre du registre.
//
// Une fabrique reçoit :
//   p       le paramètre tel que déclaré dans traitements.json (nom, label, min, max, options…)
//   valeur  sa valeur courante
//   maj     fonction à appeler avec la nouvelle valeur à chaque modification
//   id      identifiant à donner au champ principal (relié à son <label>)
// et renvoie { elements: [nœuds DOM], aide?: { texte, mono? }, sansEtiquette?: true }.
//
// Ajouter un type de paramètre = ajouter une fabrique ici ET le type côté serveur
// (voir DEVELOPPEMENT.md, « Ajouter un type de paramètre »).
// ---------------------------------------------------------------------

const CONTROLES = {
  int: (p, valeur, maj, id) => controleNombre(p, valeur, maj, id, true),
  float: (p, valeur, maj, id) => controleNombre(p, valeur, maj, id, false),
  bool: controleBool,
  choix: controleChoix,
  liste: controleListe,
};

/** Construit le bloc complet d'un paramètre : étiquette + contrôle + texte d'aide. */
function creerControle(p, valeurs) {
  const bloc = document.createElement("div");
  bloc.className = "param";
  const id = `param-${p.nom}`;
  const maj = (v) => { valeurs[p.nom] = v; };

  const fabrique = CONTROLES[p.type];
  if (!fabrique) {
    bloc.innerHTML = `<span class="param-aide">Paramètre « ${echapper(p.nom)} » : type « ${echapper(p.type)} » non géré par l'interface.</span>`;
    return bloc;
  }
  const { elements, aide, sansEtiquette } = fabrique(p, valeurs[p.nom], maj, id);

  if (!sansEtiquette) {
    const entete = document.createElement("div");
    entete.className = "param-entete";
    entete.innerHTML = `<label for="${id}">${echapper(p.label || p.nom)}</label>`;
    bloc.appendChild(entete);
  }
  bloc.append(...elements);
  // aide générée par la fabrique (bornes, taille…), puis aide écrite dans le registre (champ "aide")
  for (const a of [aide, p.aide && { texte: p.aide }]) {
    if (!a) continue;
    const span = document.createElement("span");
    span.className = a.mono ? "param-aide mono" : "param-aide";
    span.textContent = a.texte;
    bloc.appendChild(span);
  }
  return bloc;
}

/** int / float : curseur + champ numérique si min et max sont donnés, champ numérique seul sinon. */
function controleNombre(p, valeur, maj, id, entier) {
  const bornes = typeof p.min === "number" && typeof p.max === "number";
  const lire = (texte) => (entier ? parseInt(texte, 10) : parseFloat(texte));
  const ligne = document.createElement("div");
  ligne.className = "param-ligne";

  const nombre = document.createElement("input");
  nombre.type = "number";
  nombre.id = id;
  nombre.className = "champ champ-nombre";
  nombre.value = valeur;
  nombre.step = p.pas ?? (entier ? 1 : "any");
  if (typeof p.min === "number") nombre.min = p.min;
  if (typeof p.max === "number") nombre.max = p.max;
  nombre.required = true;

  if (bornes) {
    const curseur = document.createElement("input");
    curseur.type = "range";
    curseur.min = p.min;
    curseur.max = p.max;
    curseur.step = p.pas ?? (entier ? 1 : (p.max - p.min) / 100);
    curseur.value = valeur;
    curseur.setAttribute("aria-label", p.label || p.nom);
    const remplir = () => curseur.style.setProperty("--remplissage", `${((curseur.value - p.min) / (p.max - p.min || 1)) * 100}%`);
    remplir();
    curseur.addEventListener("input", () => { nombre.value = curseur.value; maj(lire(curseur.value)); remplir(); });
    nombre.addEventListener("input", () => { curseur.value = nombre.value; remplir(); });
    ligne.appendChild(curseur);
  }
  nombre.addEventListener("input", () => {
    const v = lire(nombre.value);
    if (Number.isFinite(v)) maj(v);
  });
  ligne.appendChild(nombre);

  const texte = [bornes && `${p.min} … ${p.max}`, p.pas && `pas ${p.pas}`].filter(Boolean).join(" · ");
  return { elements: [ligne], aide: texte ? { texte, mono: true } : null };
}

/** bool : case à cocher, l'étiquette est à gauche sur la même ligne. */
function controleBool(p, valeur, maj, id) {
  const lab = document.createElement("label");
  lab.className = "interrupteur";
  lab.innerHTML = `<span>${echapper(p.label || p.nom)}</span>`;
  const c = document.createElement("input");
  c.type = "checkbox";
  c.id = id;
  c.checked = !!valeur;
  c.addEventListener("change", () => maj(c.checked));
  lab.appendChild(c);
  return { elements: [lab], sansEtiquette: true };
}

/** choix : boutons si 4 options courtes au plus, liste déroulante sinon. */
function controleChoix(p, valeur, maj, id) {
  const courts = p.options.length <= 4 && p.options.every((o) => o.length <= 14);
  if (!courts) {
    const s = document.createElement("select");
    s.className = "champ champ-large";
    s.id = id;
    for (const o of p.options) s.add(new Option(o, o, false, o === valeur));
    s.addEventListener("change", () => maj(s.value));
    return { elements: [s] };
  }
  const groupe = document.createElement("div");
  groupe.className = "options";
  groupe.setAttribute("role", "radiogroup");
  groupe.id = id;
  let courant = valeur;
  const activer = () => $$("button", groupe).forEach((x) => {
    x.classList.toggle("actif", x.textContent === courant);
    x.setAttribute("aria-checked", x.textContent === courant);
  });
  for (const o of p.options) {
    const b = document.createElement("button");
    b.type = "button";
    b.textContent = o;
    b.setAttribute("role", "radio");
    b.addEventListener("click", () => { courant = o; maj(o); activer(); });
    groupe.appendChild(b);
  }
  activer();
  return { elements: [groupe] };
}

/** liste : éditeur de matrice si la taille est un carré (9, 25, 49…), champ texte sinon. */
function controleListe(p, valeur, maj, id) {
  const n = p.taille;
  const cote = n ? Math.round(Math.sqrt(n)) : 0;
  const nombres = Array.isArray(valeur) ? valeur : String(valeur).split(/[,;\s]+/).filter(Boolean).map(Number);

  if (n && cote * cote === n && cote <= 9) {
    const grille = document.createElement("div");
    grille.className = "matrice";
    grille.style.setProperty("--n", cote);
    grille.id = id;
    const cellules = [];
    for (let i = 0; i < n; i++) {
      const c = document.createElement("input");
      c.type = "number";
      c.step = "any";
      c.required = true;
      c.className = "champ";
      c.value = nombres[i] ?? 0;
      c.setAttribute("aria-label", `${p.label || p.nom} [${Math.floor(i / cote) + 1}, ${(i % cote) + 1}]`);
      c.addEventListener("input", () => maj(cellules.map((x) => parseFloat(x.value))));
      cellules.push(c);
      grille.appendChild(c);
    }
    maj(cellules.map((x) => parseFloat(x.value)));
    return { elements: [grille], aide: { texte: `matrice ${cote} × ${cote}`, mono: true } };
  }

  const champ = document.createElement("input");
  champ.type = "text";
  champ.id = id;
  champ.className = "champ champ-large";
  champ.value = nombres.join(", ");
  champ.required = true;
  champ.pattern = "\\s*-?[0-9.eE+]+(\\s*[,; ]\\s*-?[0-9.eE+]+)*\\s*";
  champ.addEventListener("input", () => maj(champ.value));
  return { elements: [champ], aide: { texte: `Nombres séparés par des virgules${n ? ` (${n} valeurs)` : ""}.` } };
}

// =====================================================================
// Emplacements d'entrée
// =====================================================================

function majEmplacements() {
  const t = traitementChoisi();
  const specs = t ? t.entrees : [{ nom: "Image", type: "tous" }];
  const nb = specs.length;

  cartesEntree.forEach((carte, i) => {
    carte.hidden = i >= nb;
    if (i >= nb) return;
    const spec = specs[i];
    $(".nom-emplacement", carte).textContent = nb > 1 ? `Image ${LETTRES[i]}` : "Image";
    // le nom déclaré n'est affiché que s'il apporte une information (« Masque », « Référence »…)
    const nomSpec = spec.nom && !/^image\b/i.test(spec.nom) ? spec.nom : "";
    $(".sous-titre", carte).textContent = [nomSpec, t ? LIBELLE_TYPE[spec.type] : ""].filter(Boolean).join(" · ");
    afficherEntree(i);
  });

  const colonnes = Math.min(nb + 1, 3);
  const grille = $("#grille-images");
  grille.style.setProperty("--colonnes", colonnes);
  grille.dataset.disposition = etat.disposition;

  $("#info-entrees").textContent = nb === 0 ? "aucune image" : `${nb} image${nb > 1 ? "s" : ""}`;
  const infoSortie = $("#info-sortie");
  infoSortie.hidden = !t;
  if (t) infoSortie.textContent = `sortie : ${LIBELLE_SORTIE[t.sortie].toLowerCase()}`;
  $('[data-vers="1"]').hidden = nb < 2;
  $('[data-vers="0"]').hidden = nb < 1;
  majBoutonAppliquer();
}

/** Prépare une image pour le type attendu. Retourne { img, note } ou lève une erreur explicite. */
function preparer(img, type, lettre) {
  switch (type) {
    case "gris":
      return { img: versGris(img), note: img.canaux === 3 ? "convertie en gris" : "" };
    case "couleur":
      return { img: versCouleur(img), note: img.canaux === 1 ? "gris → RGB" : "" };
    case "binaire": {
      const g = versGris(img);
      if (!estBinaire(g)) {
        throw new Error(`L'image ${lettre} n'est pas binaire (pixels autres que 0 et 255). ` +
          "Appliquez d'abord un seuillage, puis utilisez le résultat comme entrée.");
      }
      return { img: g, note: "" };
    }
    default:
      return { img, note: "" };
  }
}

function afficherEntree(i) {
  const carte = cartesEntree[i];
  const e = etat.entrees[i];
  const canvas = $("canvas", carte);
  const badge = $(".badge-image", carte);
  $(".vide", carte).hidden = !!e;
  canvas.hidden = !e;
  badge.hidden = !e;
  if (!e) {
    $(".meta", carte).textContent = "—";
    return;
  }
  dessiner(canvas, e.img);

  const t = traitementChoisi();
  const lettre = LETTRES[i];
  let texteBadge = `Entrée ${lettre} · ${e.img.canaux === 1 ? (estBinaire(e.img) ? "binaire" : "gris") : "RGB"}`;
  badge.classList.remove("puce-alerte");
  if (t && t.entrees[i]) {
    try {
      const { note } = preparer(e.img, t.entrees[i].type, lettre);
      if (note) texteBadge += ` → ${note}`;
    } catch {
      texteBadge += " → non binaire !";
      badge.classList.add("puce-alerte");
    }
  }
  badge.textContent = texteBadge;
  const parties = [`${e.img.largeur} × ${e.img.hauteur} px`, e.img.canaux === 1 ? "Gris 8 bits" : "RGB 24 bits", e.nom];
  if (e.taille) parties.push(formaterTaille(e.taille));
  $(".meta", carte).textContent = parties.join("  ·  ");
}

function dessiner(canvas, img) {
  canvas.width = img.largeur;
  canvas.height = img.hauteur;
  canvas.getContext("2d").putImageData(versImageData(img), 0, 0);
  // Les petites images sont agrandies (sans lissage) pour rester lisibles
  const facteur = Math.floor(200 / Math.max(img.largeur, img.hauteur));
  canvas.style.width = facteur > 1 ? `${img.largeur * facteur}px` : "";
}

function formaterTaille(o) {
  if (o < 1024) return `${o} o`;
  if (o < 1024 ** 2) return `${(o / 1024).toFixed(0)} Ko`;
  return `${(o / 1024 ** 2).toFixed(1)} Mo`;
}

async function importer(i, fichier) {
  if (!fichier) return;
  retirerBandeau("fichier");
  try {
    const img = await chargerFichier(fichier);
    if (img.largeur > DIMENSION_MAX || img.hauteur > DIMENSION_MAX) {
      throw new Error(`image trop grande (${img.largeur} × ${img.hauteur}, maximum ${DIMENSION_MAX})`);
    }
    etat.entrees[i] = { img, nom: fichier.name, taille: fichier.size };
    afficherEntree(i);
    majBoutonAppliquer();
  } catch (e) {
    afficherBandeau("fichier", "erreur", `Impossible de lire « ${echapper(fichier.name)} » : ${echapper(e.message)}`);
  }
}

cartesEntree.forEach((carte, i) => {
  const input = $('input[type="file"]', carte);
  const zone = $(".zone-image", carte);
  input.addEventListener("change", () => { importer(i, input.files[0]); input.value = ""; });
  zone.addEventListener("click", () => { if (!etat.entrees[i]) input.click(); });
  zone.addEventListener("keydown", (ev) => {
    if ((ev.key === "Enter" || ev.key === " ") && ev.target === zone) { ev.preventDefault(); input.click(); }
  });
  zone.addEventListener("dragover", (ev) => { ev.preventDefault(); zone.classList.add("survol-depot"); });
  zone.addEventListener("dragleave", () => zone.classList.remove("survol-depot"));
  zone.addEventListener("drop", (ev) => {
    ev.preventDefault();
    zone.classList.remove("survol-depot");
    importer(i, ev.dataTransfer.files[0]);
  });
  inspecteurPixels($("canvas", carte), $(".pixel", carte), () => etat.entrees[i]?.img);
});

// Empêche le navigateur d'ouvrir un fichier lâché à côté d'une zone de dépôt
window.addEventListener("dragover", (ev) => ev.preventDefault());
window.addEventListener("drop", (ev) => ev.preventDefault());

/** Affiche les coordonnées et la valeur du pixel survolé. */
function inspecteurPixels(canvas, sortie, image) {
  canvas.addEventListener("pointermove", (ev) => {
    const img = image();
    if (!img) return;
    const r = canvas.getBoundingClientRect();
    const x = Math.min(img.largeur - 1, Math.max(0, Math.floor(((ev.clientX - r.left) / r.width) * img.largeur)));
    const y = Math.min(img.hauteur - 1, Math.max(0, Math.floor(((ev.clientY - r.top) / r.height) * img.hauteur)));
    const k = y * img.largeur + x;
    const v = img.canaux === 1
      ? `${img.pixels[k]}`
      : `R ${img.pixels[3 * k]}  G ${img.pixels[3 * k + 1]}  B ${img.pixels[3 * k + 2]}`;
    sortie.textContent = `x ${x}  y ${y}  →  ${v}`;
  });
  canvas.addEventListener("pointerleave", () => { sortie.textContent = ""; });
}

// =====================================================================
// Exécution d'un traitement
// =====================================================================

function verifierPret() {
  const t = traitementChoisi();
  if (!t) return "Choisissez un traitement.";
  if (!t.disponible) return t.raison || "Ce traitement n'est pas encore compilé.";
  for (let i = 0; i < t.entrees.length; i++) {
    if (!etat.entrees[i]) return `Importez ${t.entrees.length > 1 ? `l'image ${LETTRES[i]}` : "une image"}.`;
  }
  return "";
}

function majBoutonAppliquer() {
  const raison = etat.enCours ? "Traitement en cours…" : verifierPret();
  const bouton = $("#btn-appliquer");
  bouton.disabled = !!raison;
  const aide = $("#aide-action");
  aide.classList.toggle("alerte", !!raison && !!traitementChoisi() && !traitementChoisi().disponible);
  aide.innerHTML = raison ? echapper(raison) : "Raccourci : <kbd>Ctrl</kbd> + <kbd>Entrée</kbd>";
}

async function appliquer() {
  const t = traitementChoisi();
  if (etat.enCours || verifierPret() || !t) return;
  const form = $("#form-params");
  if (!form.checkValidity()) { form.reportValidity(); return; }

  // 1. Préparer les images au format attendu
  let images;
  try {
    images = t.entrees.map((spec, i) => {
      const { img } = preparer(etat.entrees[i].img, spec.type, LETTRES[i]);
      return versBase64(encoderPNM(img));
    });
  } catch (e) {
    afficherErreurResultat(e.message, "");
    return;
  }
  if (t.entrees.length === 2) {
    const [a, b] = etat.entrees;
    if (a.img.largeur !== b.img.largeur || a.img.hauteur !== b.img.hauteur) {
      afficherBandeau("tailles", "alerte",
        `Les images A (${a.img.largeur} × ${a.img.hauteur}) et B (${b.img.largeur} × ${b.img.hauteur}) n'ont pas la même taille : ` +
        "le résultat dépend de la façon dont le traitement gère ce cas.");
    } else retirerBandeau("tailles");
  }

  // 2. Envoyer au serveur
  etat.enCours = true;
  majBoutonAppliquer();
  const zone = $(".zone-resultat");
  zone.hidden = false;
  $("#chargement").hidden = false;
  const debut = performance.now();
  let rep, donnees;
  try {
    rep = await fetch(`/api/traitements/${encodeURIComponent(t.id)}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ images, params: valeursCourantes(t) }),
    });
    donnees = await rep.json().catch(() => ({ erreur: `réponse illisible du serveur (HTTP ${rep.status})` }));
  } catch (e) {
    donnees = { erreur: "le serveur ne répond pas", details: "Vérifiez qu'il est toujours démarré (voir README)." };
    rep = { ok: false, status: 0 };
  } finally {
    etat.enCours = false;
    $("#chargement").hidden = true;
    majBoutonAppliquer();
  }

  // 3. Afficher le résultat ou l'erreur
  if (!rep.ok) {
    afficherErreurResultat(donnees.erreur || `erreur HTTP ${rep.status}`, donnees.details || "");
    journaliser(t, { ok: false, statut: rep.status, ...donnees, duree: performance.now() - debut });
    if (rep.status === 503) chargerRegistre();
    return;
  }
  try {
    afficherResultat(t, donnees);
    journaliser(t, { ok: true, statut: 200, ...donnees });
  } catch (e) {
    afficherErreurResultat(`résultat illisible : ${e.message}`, "");
    journaliser(t, { ok: false, statut: 200, erreur: e.message, ...donnees });
  }
}

function afficherResultat(t, donnees) {
  const img = donnees.image ? decoderPNM(depuisBase64(donnees.image.donnees)) : null;
  etat.resultat = { img, json: donnees.json, traitement: t };

  const zone = $(".zone-resultat");
  const canvas = $("canvas", zone);
  const badge = $(".badge-image", zone);
  $("#resultat-vide").hidden = true;
  $("#erreur-resultat").hidden = true;
  zone.hidden = !img;
  canvas.hidden = !img;
  badge.hidden = !img;
  if (img) {
    dessiner(canvas, img);
    badge.textContent = `Sortie · ${img.canaux === 1 ? (estBinaire(img) ? "binaire" : "gris") : "RGB"}`;
  }

  const zoneJSON = $("#resultat-json");
  zoneJSON.hidden = donnees.json === undefined;
  if (donnees.json !== undefined) afficherJSON(zoneJSON, donnees.json);

  const badgeT = $("#badge-traitement");
  badgeT.hidden = false;
  badgeT.textContent = t.nom;
  const meta = [img && `${img.largeur} × ${img.hauteur} px`, img && (img.canaux === 1 ? "PGM" : "PPM"), `${donnees.duree_ms} ms`];
  $(".meta", carteResultat).textContent = meta.filter(Boolean).join("  ·  ");
  // pas d'image (sortie JSON seule) : rien à enchaîner ni à télécharger
  $("#actions-resultat").hidden = !img;
  $("#btn-telecharger-pnm").textContent = img && img.canaux === 3 ? "PPM" : "PGM";
}

function afficherErreurResultat(message, details, t = traitementChoisi()) {
  etat.resultat = null;
  $("#badge-traitement").hidden = !t;
  if (t) $("#badge-traitement").textContent = t.nom;
  const zone = $(".zone-resultat");
  zone.hidden = false;
  $("canvas", zone).hidden = true;
  $(".badge-image", zone).hidden = true;
  $("#resultat-vide").hidden = true;
  $("#resultat-json").hidden = true;
  const boite = $("#erreur-resultat");
  boite.hidden = false;
  $("strong", boite).textContent = message;
  $("pre", boite).textContent = details || "";
  $("#actions-resultat").hidden = true;
  $(".meta", carteResultat).textContent = "—";
}

function viderResultat() {
  etat.resultat = null;
  const zone = $(".zone-resultat");
  zone.hidden = false;
  $("canvas", zone).hidden = true;
  $(".badge-image", zone).hidden = true;
  $("#resultat-vide").hidden = false;
  $("#erreur-resultat").hidden = true;
  $("#resultat-json").hidden = true;
  $("#badge-traitement").hidden = true;
  $("#actions-resultat").hidden = true;
  $(".meta", carteResultat).textContent = "—";
}

inspecteurPixels($("canvas", carteResultat), $(".pixel", carteResultat), () => etat.resultat?.img);

// =====================================================================
// Journal des exécutions
// =====================================================================

function journaliser(t, r) {
  const corps = $("#journal-corps");
  const entree = document.createElement("div");
  entree.className = `journal-entree ${r.ok ? "succes" : "echec"}`;
  const heure = new Date().toLocaleTimeString("fr-FR");
  const duree = r.duree_ms ?? (r.duree ? Math.round(r.duree) : null);
  const statut = r.ok ? "OK" : `Erreur${r.statut ? ` ${r.statut}` : ""}`;
  let html = `<div class="ligne-entre"><span><b>${echapper(t.nom)}</b> <span class="mono discret">${heure}</span></span>` +
    `<span class="statut mono">${statut}${duree !== null ? ` · ${duree} ms` : ""}</span></div>`;
  if (r.commande) html += `<code>$ ${echapper(r.commande)}</code>`;
  if (!r.ok && r.erreur) html += `<pre>${echapper(r.erreur)}${r.details ? `\n\n${echapper(r.details)}` : ""}</pre>`;
  if (r.stdout) html += `<pre>stdout :\n${echapper(r.stdout)}</pre>`;
  if (r.stderr && r.ok) html += `<pre>stderr :\n${echapper(r.stderr)}</pre>`;
  entree.innerHTML = html;
  corps.prepend(entree);
  while (corps.children.length > 30) corps.lastElementChild.remove();
  $("#journal-resume").textContent = `dernier : ${t.nom} · ${statut}${duree !== null ? ` · ${duree} ms` : ""}`;
  if (!r.ok) $("#journal").open = true;
}

// =====================================================================
// Actions diverses
// =====================================================================

$$("[data-vers]").forEach((b) => b.addEventListener("click", () => {
  if (!etat.resultat?.img) return;
  const i = Number(b.dataset.vers);
  etat.entrees[i] = { img: etat.resultat.img, nom: `résultat « ${etat.resultat.traitement.nom} »`, taille: 0 };
  afficherEntree(i);
  majBoutonAppliquer();
}));

function telecharger(blob, nom) {
  const url = URL.createObjectURL(blob);
  const a = Object.assign(document.createElement("a"), { href: url, download: nom });
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

$("#btn-telecharger-png").addEventListener("click", () => {
  if (!etat.resultat?.img) return;
  $("canvas", carteResultat).toBlob((blob) => telecharger(blob, `${etat.resultat.traitement.id}.png`), "image/png");
});
$("#btn-telecharger-pnm").addEventListener("click", () => {
  const img = etat.resultat?.img;
  if (!img) return;
  telecharger(new Blob([encoderPNM(img)], { type: "image/x-portable-anymap" }),
    `${etat.resultat.traitement.id}.${img.canaux === 1 ? "pgm" : "ppm"}`);
});

$("#btn-reinitialiser").addEventListener("click", () => {
  etat.entrees = [null, null];
  cartesEntree.forEach((_, i) => afficherEntree(i));
  viderResultat();
  ["fichier", "tailles"].forEach(retirerBandeau);
  majBoutonAppliquer();
});

$("#btn-defauts").addEventListener("click", () => {
  const t = traitementChoisi();
  if (!t) return;
  delete etat.valeursParams[t.id];
  afficherParams();
});

$("#btn-appliquer").addEventListener("click", appliquer);
$("#form-params").addEventListener("submit", (ev) => { ev.preventDefault(); appliquer(); });
document.addEventListener("keydown", (ev) => {
  if ((ev.ctrlKey || ev.metaKey) && ev.key === "Enter") { ev.preventDefault(); appliquer(); }
});

$("#recherche").addEventListener("input", (ev) => { etat.recherche = ev.target.value; afficherListe(); });
$$("[data-defiler]").forEach((b) => b.addEventListener("click", () => {
  $("#chapitres").scrollBy({ left: Number(b.dataset.defiler) * 160, behavior: "smooth" });
}));

// Boutons Grille / Empilé. Sélecteur limité aux boutons : la grille d'images porte aussi
// un attribut data-disposition, et un clic dedans ne doit pas changer la disposition.
const boutonsDisposition = $$(".segmente button[data-disposition]");

function appliquerDisposition(disposition) {
  etat.disposition = disposition;
  for (const b of boutonsDisposition) {
    const actif = b.dataset.disposition === disposition;
    b.classList.toggle("actif", actif);
    b.setAttribute("aria-pressed", actif);
  }
  majEmplacements();
  if (etat.resultat?.json !== undefined && !$("#resultat-json").hidden) afficherJSON($("#resultat-json"), etat.resultat.json);
}

boutonsDisposition.forEach((b) => b.addEventListener("click", () => appliquerDisposition(b.dataset.disposition)));

$("#btn-actualiser").addEventListener("click", () => { etat.signatureRegistre = ""; chargerRegistre(); });
// Quand on revient sur l'onglet (après avoir compilé un traitement, par exemple), on relit le registre
window.addEventListener("focus", () => chargerRegistre());

// Redessiner les graphiques quand la largeur change
let minuterie;
window.addEventListener("resize", () => {
  clearTimeout(minuterie);
  minuterie = setTimeout(() => {
    if (etat.resultat?.json !== undefined && !$("#resultat-json").hidden) afficherJSON($("#resultat-json"), etat.resultat.json);
  }, 150);
});

// =====================================================================
// Thème clair / sombre
// =====================================================================
// Sans choix enregistré, on suit le thème du système. Le choix est mémorisé dans le
// navigateur (localStorage) ; il est appliqué avant l'affichage par un script de index.html.

const requeteSombre = window.matchMedia("(prefers-color-scheme: dark)");
const themeEffectif = () => document.documentElement.dataset.theme || (requeteSombre.matches ? "dark" : "light");

function majBoutonTheme() {
  const sombre = themeEffectif() === "dark";
  const bouton = $("#btn-theme");
  bouton.dataset.sombre = sombre;
  const texte = sombre ? "Passer en mode clair" : "Passer en mode sombre";
  bouton.title = texte;
  bouton.setAttribute("aria-label", texte);
}

const moinsDAnimations = window.matchMedia("(prefers-reduced-motion: reduce)");

$("#btn-theme").addEventListener("click", () => {
  const nouveau = themeEffectif() === "dark" ? "light" : "dark";
  const racine = document.documentElement;
  const bouton = $("#btn-theme");
  try { localStorage.setItem("theme", nouveau); } catch { /* stockage indisponible : choix non mémorisé */ }

  // Les transitions de couleur sont coupées pendant le changement : sinon chaque
  // élément changerait à son rythme et on verrait un mélange des deux thèmes.
  const basculer = () => {
    racine.dataset.theme = nouveau;
    majBoutonTheme();
  };
  racine.classList.add("changement-theme");

  // Effet : le nouveau thème apparaît dans un cercle qui grandit depuis le bouton.
  // View Transitions API ; sans elle (ou si le système demande moins d'animations) : bascule directe.
  if (!document.startViewTransition || moinsDAnimations.matches) {
    basculer();
    requestAnimationFrame(() => requestAnimationFrame(() => racine.classList.remove("changement-theme")));
    return;
  }
  const r = bouton.getBoundingClientRect();
  const x = r.left + r.width / 2;
  const y = r.top + r.height / 2;
  const rayon = Math.hypot(Math.max(x, innerWidth - x), Math.max(y, innerHeight - y));

  bouton.classList.remove("tourne");
  void bouton.offsetWidth; // relance l'animation de l'icône
  bouton.classList.add("tourne");

  const transition = document.startViewTransition(basculer);
  transition.ready.then(() => {
    racine.animate(
      { clipPath: [`circle(0px at ${x}px ${y}px)`, `circle(${rayon}px at ${x}px ${y}px)`] },
      { duration: 500, easing: "cubic-bezier(0.4, 0, 0.2, 1)", pseudoElement: "::view-transition-new(root)" },
    );
  });
  transition.finished.finally(() => racine.classList.remove("changement-theme"));
});
requeteSombre.addEventListener("change", majBoutonTheme);

// Démarrage
majBoutonTheme();
viderResultat();
appliquerDisposition(etat.disposition); // « Grille » sélectionné par défaut
chargerRegistre();
