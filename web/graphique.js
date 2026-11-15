// Affichage des résultats JSON des traitements (histogramme, profil, statistiques).
//
// Format attendu (voir DEVELOPPEMENT.md, « Sortie JSON ») :
//   { "courbes": [{ "nom", "valeurs", "x"?, "couleur"?, "style"?: "ligne" | "barres" }],
//     "valeurs": { "cle": valeur, ... } }
// Toute autre clé est affichée telle quelle dans « Données brutes ».

const NS = "http://www.w3.org/2000/svg";
// Ordre catégoriel fixe (jamais cyclé) pour les courbes sans couleur imposée.
// Variables CSS (style.css) : les couleurs s'adaptent au thème clair / sombre.
const PALETTE = Array.from({ length: 8 }, (_, i) => `var(--serie-${i + 1})`);

function el(tag, attrs = {}, parent) {
  const n = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) n.setAttribute(k, v);
  if (parent) parent.appendChild(n);
  return n;
}

function formater(v) {
  if (typeof v !== "number") return String(v);
  if (Number.isInteger(v)) return v.toLocaleString("fr-FR");
  return v.toLocaleString("fr-FR", { maximumFractionDigits: 3 });
}

/** Graduations « rondes » entre 0 et max. */
function graduations(max, nb = 4) {
  if (max <= 0) return [0];
  const pasBrut = max / nb;
  const puissance = 10 ** Math.floor(Math.log10(pasBrut));
  const pas = [1, 2, 2.5, 5, 10].map((m) => m * puissance).find((p) => p >= pasBrut);
  // on va jusqu'à la première graduation ≥ max, pour que les courbes ne dépassent pas l'axe
  const res = [0];
  while (res[res.length - 1] < max - pas * 1e-9) res.push(+(res.length * pas).toPrecision(12));
  return res;
}

/** Trace les courbes dans `conteneur` (SVG avec infobulle au survol). */
export function tracerCourbes(conteneur, courbes) {
  const series = courbes
    .filter((c) => c && Array.isArray(c.valeurs) && c.valeurs.length)
    .map((c, i) => ({
      nom: c.nom ?? `Série ${i + 1}`,
      valeurs: c.valeurs.map(Number),
      x: Array.isArray(c.x) && c.x.length === c.valeurs.length ? c.x.map(Number) : c.valeurs.map((_, j) => j),
      couleur: c.couleur || PALETTE[i % PALETTE.length],
      barres: c.style === "barres",
    }));
  if (!series.length) return;

  const bloc = document.createElement("div");
  bloc.className = "graphique";
  conteneur.appendChild(bloc);

  // Légende : seulement à partir de 2 séries (une seule : le titre suffit)
  if (series.length > 1) {
    const legende = document.createElement("div");
    legende.className = "legende";
    for (const s of series) {
      const item = document.createElement("span");
      item.innerHTML = `<i style="background:${s.couleur}"></i>`;
      item.append(s.nom);
      legende.appendChild(item);
    }
    bloc.appendChild(legende);
  } else {
    const titre = document.createElement("div");
    titre.className = "graphique-titre";
    titre.textContent = series[0].nom;
    bloc.appendChild(titre);
  }

  const zone = document.createElement("div");
  zone.className = "graphique-zone";
  bloc.appendChild(zone);

  const L = Math.max(zone.clientWidth || conteneur.clientWidth || 600, 280);
  const H = 220;
  const m = { haut: 10, droite: 12, bas: 26, gauche: 52 };
  const largeurUtile = L - m.gauche - m.droite;
  const hauteurUtile = H - m.haut - m.bas;

  const toutesX = series.flatMap((s) => s.x);
  const xmin = Math.min(...toutesX);
  const xmax = Math.max(...toutesX);
  const ymaxBrut = Math.max(0, ...series.flatMap((s) => s.valeurs.filter(Number.isFinite)));
  const yminBrut = Math.min(0, ...series.flatMap((s) => s.valeurs.filter(Number.isFinite)));
  const ticks = graduations(ymaxBrut - yminBrut);
  const ymin = yminBrut;
  const ymax = ymin + (ticks[ticks.length - 1] || 1);

  const sx = (x) => m.gauche + (xmax === xmin ? largeurUtile / 2 : ((x - xmin) / (xmax - xmin)) * largeurUtile);
  const sy = (y) => m.haut + hauteurUtile - ((y - ymin) / (ymax - ymin || 1)) * hauteurUtile;

  const svg = el("svg", { viewBox: `0 0 ${L} ${H}`, width: L, height: H, role: "img" }, zone);
  svg.setAttribute("aria-label", series.map((s) => s.nom).join(", "));

  // Grille horizontale discrète + graduations Y
  for (const t of ticks) {
    const y = sy(ymin + t);
    el("line", { x1: m.gauche, x2: L - m.droite, y1: y, y2: y, class: "grille-ligne" }, svg);
    el("text", { x: m.gauche - 8, y: y + 4, class: "axe-texte", "text-anchor": "end" }, svg).textContent = formater(ymin + t);
  }
  // Graduations X (5 valeurs)
  for (let i = 0; i <= 4; i++) {
    const xv = xmin + ((xmax - xmin) * i) / 4;
    el("text", { x: sx(xv), y: H - 8, class: "axe-texte", "text-anchor": i === 0 ? "start" : i === 4 ? "end" : "middle" }, svg)
      .textContent = formater(Number.isInteger(xmin) && Number.isInteger(xmax) ? Math.round(xv) : xv);
  }
  el("line", { x1: m.gauche, x2: L - m.droite, y1: sy(Math.max(ymin, 0)), y2: sy(Math.max(ymin, 0)), class: "axe-ligne" }, svg);

  // Tracés
  for (const s of series) {
    if (s.barres) {
      const pas = s.x.length > 1 ? Math.abs(sx(s.x[1]) - sx(s.x[0])) : largeurUtile;
      const ecart = pas > 4 ? 1 : 0;
      const g = el("g", { style: `fill: ${s.couleur}` }, svg);
      s.valeurs.forEach((v, i) => {
        if (!Number.isFinite(v)) return;
        const y0 = sy(0);
        const y1 = sy(v);
        el("rect", { x: sx(s.x[i]) - pas / 2 + ecart / 2, y: Math.min(y0, y1), width: Math.max(pas - ecart, 0.5), height: Math.abs(y0 - y1) }, g);
      });
    } else {
      const points = s.valeurs.map((v, i) => (Number.isFinite(v) ? `${sx(s.x[i]).toFixed(2)},${sy(v).toFixed(2)}` : null)).filter(Boolean);
      if (series.length === 1) {
        const base = sy(Math.max(ymin, 0)).toFixed(2);
        el("polygon", { points: `${points[0].split(",")[0]},${base} ${points.join(" ")} ${points[points.length - 1].split(",")[0]},${base}`, style: `fill: ${s.couleur}`, opacity: 0.1 }, svg);
      }
      el("polyline", { points: points.join(" "), fill: "none", style: `stroke: ${s.couleur}`, "stroke-width": 2, "stroke-linejoin": "round", "stroke-linecap": "round" }, svg);
    }
  }

  // Survol : réticule + infobulle avec la valeur de chaque série
  const reticule = el("line", { y1: m.haut, y2: m.haut + hauteurUtile, class: "reticule", visibility: "hidden" }, svg);
  const marqueurs = series.map((s) => el("circle", { r: 4, style: `fill: ${s.couleur}; stroke: var(--surface)`, "stroke-width": 2, visibility: "hidden" }, svg));
  const bulle = document.createElement("div");
  bulle.className = "infobulle";
  bulle.hidden = true;
  zone.appendChild(bulle);
  const capteur = el("rect", { x: m.gauche, y: m.haut, width: largeurUtile, height: hauteurUtile, fill: "transparent" }, svg);

  capteur.addEventListener("pointermove", (ev) => {
    const r = svg.getBoundingClientRect();
    const px = ((ev.clientX - r.left) / r.width) * L;
    const xv = xmin + ((px - m.gauche) / largeurUtile) * (xmax - xmin);
    const ref = series[0];
    let idx = 0;
    for (let i = 1; i < ref.x.length; i++) if (Math.abs(ref.x[i] - xv) < Math.abs(ref.x[idx] - xv)) idx = i;
    const xp = sx(ref.x[idx]);
    reticule.setAttribute("x1", xp);
    reticule.setAttribute("x2", xp);
    reticule.setAttribute("visibility", "visible");
    series.forEach((s, k) => {
      const v = s.valeurs[idx];
      const ok = Number.isFinite(v) && !s.barres;
      marqueurs[k].setAttribute("visibility", ok ? "visible" : "hidden");
      if (ok) { marqueurs[k].setAttribute("cx", sx(s.x[idx])); marqueurs[k].setAttribute("cy", sy(v)); }
    });
    bulle.innerHTML = `<div class="infobulle-x">x = ${formater(ref.x[idx])}</div>` +
      series.map((s) => `<div><i style="background:${s.couleur}"></i><span>${echapper(s.nom)}</span><b>${formater(s.valeurs[idx])}</b></div>`).join("");
    bulle.hidden = false;
    const zr = zone.getBoundingClientRect();
    const gauche = (xp / L) * zr.width;
    bulle.style.left = `${Math.min(Math.max(gauche + 12, 0), zr.width - bulle.offsetWidth)}px`;
    if (gauche + 12 + bulle.offsetWidth > zr.width) bulle.style.left = `${Math.max(gauche - 12 - bulle.offsetWidth, 0)}px`;
  });
  capteur.addEventListener("pointerleave", () => {
    reticule.setAttribute("visibility", "hidden");
    marqueurs.forEach((mk) => mk.setAttribute("visibility", "hidden"));
    bulle.hidden = true;
  });
}

function echapper(t) {
  return String(t).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
}

/** Affiche un objet JSON de résultat complet dans `conteneur`. */
export function afficherJSON(conteneur, donnees) {
  conteneur.innerHTML = "";
  if (donnees === null || typeof donnees !== "object") {
    const p = document.createElement("pre");
    p.className = "json-brut";
    p.textContent = JSON.stringify(donnees, null, 2);
    conteneur.appendChild(p);
    return;
  }

  if (Array.isArray(donnees.courbes) && donnees.courbes.length) tracerCourbes(conteneur, donnees.courbes);

  if (donnees.valeurs && typeof donnees.valeurs === "object") {
    const table = document.createElement("table");
    table.className = "table-valeurs";
    for (const [k, v] of Object.entries(donnees.valeurs)) {
      const tr = table.insertRow();
      tr.insertCell().textContent = k;
      tr.insertCell().textContent = typeof v === "object" ? JSON.stringify(v) : formater(v);
    }
    conteneur.appendChild(table);
  }

  // Données brutes : vue tableau/texte accessible, et tout ce qui n'est pas reconnu
  const details = document.createElement("details");
  details.className = "json-details";
  const reconnu = Array.isArray(donnees.courbes) || donnees.valeurs;
  details.open = !reconnu;
  details.innerHTML = "<summary>Données brutes (JSON)</summary>";
  const pre = document.createElement("pre");
  pre.className = "json-brut";
  pre.textContent = JSON.stringify(donnees, null, 2);
  details.appendChild(pre);
  conteneur.appendChild(details);
}
