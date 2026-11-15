// Lecture / écriture des images PGM-PPM et conversions côté navigateur.
//
// Représentation interne d'une image : { largeur, hauteur, canaux (1 ou 3), pixels: Uint8Array }
// (pixels rangés ligne par ligne, canaux entrelacés : RGBRGB... pour la couleur).

/** Décode un fichier PNM (P1 à P6, valeur max quelconque) en image interne 8 bits. */
export function decoderPNM(octets) {
  if (octets.length < 2 || octets[0] !== 0x50) throw new Error("ce n'est pas une image PGM/PPM");
  const type = octets[1] - 0x30; // 1..6
  if (type < 1 || type > 6) throw new Error(`format P${String.fromCharCode(octets[1])} inconnu`);
  const ascii = type <= 3;
  const canaux = type === 3 || type === 6 ? 3 : 1;
  const bitmap = type === 1 || type === 4;

  let pos = 2;
  const estBlanc = (c) => c === 0x20 || c === 0x09 || c === 0x0a || c === 0x0d || c === 0x0b || c === 0x0c;
  function lireEntier() {
    for (;;) {
      while (pos < octets.length && estBlanc(octets[pos])) pos++;
      if (octets[pos] === 0x23) { // commentaire '#'
        while (pos < octets.length && octets[pos] !== 0x0a && octets[pos] !== 0x0d) pos++;
      } else break;
    }
    let debut = pos;
    while (pos < octets.length && octets[pos] >= 0x30 && octets[pos] <= 0x39) pos++;
    if (debut === pos) throw new Error("en-tête PGM/PPM invalide");
    return parseInt(new TextDecoder().decode(octets.subarray(debut, pos)), 10);
  }

  const largeur = lireEntier();
  const hauteur = lireEntier();
  const maxval = bitmap ? 1 : lireEntier();
  if (largeur <= 0 || hauteur <= 0) throw new Error(`dimensions invalides ${largeur}x${hauteur}`);
  if (maxval <= 0 || maxval > 65535) throw new Error(`valeur max invalide : ${maxval}`);

  const n = largeur * hauteur * canaux;
  const pixels = new Uint8Array(n);
  const echelle = (v) => (maxval === 255 ? v : Math.round((v * 255) / maxval));

  if (ascii) {
    for (let i = 0; i < n; i++) {
      const v = bitmap ? lireBit() : lireEntier();
      pixels[i] = bitmap ? (v ? 0 : 255) : echelle(v); // PBM : 1 = noir
    }
  } else {
    pos++; // un seul blanc après l'en-tête
    if (type === 4) {
      const parLigne = Math.ceil(largeur / 8);
      if (octets.length - pos < parLigne * hauteur) throw new Error("données tronquées");
      for (let y = 0; y < hauteur; y++)
        for (let x = 0; x < largeur; x++) {
          const bit = (octets[pos + y * parLigne + (x >> 3)] >> (7 - (x & 7))) & 1;
          pixels[y * largeur + x] = bit ? 0 : 255;
        }
    } else if (maxval < 256) {
      if (octets.length - pos < n) throw new Error("données tronquées");
      for (let i = 0; i < n; i++) pixels[i] = echelle(octets[pos + i]);
    } else {
      if (octets.length - pos < 2 * n) throw new Error("données tronquées");
      for (let i = 0; i < n; i++) pixels[i] = echelle((octets[pos + 2 * i] << 8) | octets[pos + 2 * i + 1]);
    }
  }
  return { largeur, hauteur, canaux, pixels };

  function lireBit() {
    while (pos < octets.length && (estBlanc(octets[pos]) || octets[pos] === 0x23)) {
      if (octets[pos] === 0x23) while (pos < octets.length && octets[pos] !== 0x0a) pos++;
      else pos++;
    }
    if (pos >= octets.length) throw new Error("données tronquées");
    return octets[pos++] === 0x31 ? 1 : 0;
  }
}

/** Encode une image interne en PGM (P5) ou PPM (P6) binaire, valeur max 255. */
export function encoderPNM(img) {
  const entete = new TextEncoder().encode(`P${img.canaux === 1 ? 5 : 6}\n${img.largeur} ${img.hauteur}\n255\n`);
  const sortie = new Uint8Array(entete.length + img.pixels.length);
  sortie.set(entete, 0);
  sortie.set(img.pixels, entete.length);
  return sortie;
}

/** Image interne -> ImageData (RGBA) pour l'affichage dans un <canvas>. */
export function versImageData(img) {
  const { largeur, hauteur, canaux, pixels } = img;
  const rgba = new Uint8ClampedArray(largeur * hauteur * 4);
  for (let i = 0, j = 0; i < largeur * hauteur; i++, j += 4) {
    if (canaux === 1) {
      rgba[j] = rgba[j + 1] = rgba[j + 2] = pixels[i];
    } else {
      rgba[j] = pixels[3 * i];
      rgba[j + 1] = pixels[3 * i + 1];
      rgba[j + 2] = pixels[3 * i + 2];
    }
    rgba[j + 3] = 255;
  }
  return new ImageData(rgba, largeur, hauteur);
}

/** ImageData (RGBA) -> image interne couleur (3 canaux). L'alpha est ignoré. */
export function depuisImageData(donnees) {
  const n = donnees.width * donnees.height;
  const pixels = new Uint8Array(n * 3);
  for (let i = 0; i < n; i++) {
    pixels[3 * i] = donnees.data[4 * i];
    pixels[3 * i + 1] = donnees.data[4 * i + 1];
    pixels[3 * i + 2] = donnees.data[4 * i + 2];
  }
  return { largeur: donnees.width, hauteur: donnees.height, canaux: 3, pixels };
}

/** Vrai si tous les pixels ont R = G = B (image en niveaux de gris stockée en couleur). */
export function estGris(img) {
  if (img.canaux === 1) return true;
  const p = img.pixels;
  for (let i = 0; i < p.length; i += 3) if (p[i] !== p[i + 1] || p[i] !== p[i + 2]) return false;
  return true;
}

/**
 * Convertit en niveaux de gris (1 canal).
 * Image déjà grise : copie exacte. Sinon luminance UIT-R BT.601 : Y = 0.299 R + 0.587 G + 0.114 B.
 */
export function versGris(img) {
  if (img.canaux === 1) return img;
  const n = img.largeur * img.hauteur;
  const p = img.pixels;
  const gris = new Uint8Array(n);
  for (let i = 0; i < n; i++) gris[i] = Math.round(0.299 * p[3 * i] + 0.587 * p[3 * i + 1] + 0.114 * p[3 * i + 2]);
  return { largeur: img.largeur, hauteur: img.hauteur, canaux: 1, pixels: gris };
}

/** Convertit en couleur (3 canaux) en recopiant le niveau de gris dans R, G et B. */
export function versCouleur(img) {
  if (img.canaux === 3) return img;
  const n = img.largeur * img.hauteur;
  const rgb = new Uint8Array(n * 3);
  for (let i = 0; i < n; i++) rgb[3 * i] = rgb[3 * i + 1] = rgb[3 * i + 2] = img.pixels[i];
  return { largeur: img.largeur, hauteur: img.hauteur, canaux: 3, pixels: rgb };
}

/** Vrai si l'image (1 canal) ne contient que des 0 et des 255. */
export function estBinaire(img) {
  if (img.canaux !== 1) return false;
  for (const v of img.pixels) if (v !== 0 && v !== 255) return false;
  return true;
}

/** Uint8Array -> base64 (par morceaux pour ne pas dépasser la pile). */
export function versBase64(octets) {
  let texte = "";
  const morceau = 0x8000;
  for (let i = 0; i < octets.length; i += morceau) {
    texte += String.fromCharCode.apply(null, octets.subarray(i, i + morceau));
  }
  return btoa(texte);
}

/** base64 -> Uint8Array. */
export function depuisBase64(b64) {
  const texte = atob(b64);
  const octets = new Uint8Array(texte.length);
  for (let i = 0; i < texte.length; i++) octets[i] = texte.charCodeAt(i);
  return octets;
}

/**
 * Charge un fichier choisi par l'utilisateur en image interne.
 * - PGM / PPM / PBM / PNM : décodés ici (le navigateur ne sait pas les afficher)
 * - PNG, JPEG, WebP, BMP, GIF... : décodés par le navigateur ; la transparence est posée sur fond blanc
 */
export async function chargerFichier(fichier) {
  const octets = new Uint8Array(await fichier.arrayBuffer());
  const estPNM = /\.(pgm|ppm|pbm|pnm)$/i.test(fichier.name) ||
    (octets[0] === 0x50 && octets[1] >= 0x31 && octets[1] <= 0x36 && /\s/.test(String.fromCharCode(octets[2])));
  if (estPNM) return decoderPNM(octets);

  let bitmap;
  try {
    bitmap = await createImageBitmap(new Blob([octets], { type: fichier.type || "" }));
  } catch {
    throw new Error("format d'image non reconnu par le navigateur");
  }
  const canvas = document.createElement("canvas");
  canvas.width = bitmap.width;
  canvas.height = bitmap.height;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  ctx.fillStyle = "#ffffff";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.drawImage(bitmap, 0, 0);
  bitmap.close?.();
  const img = depuisImageData(ctx.getImageData(0, 0, canvas.width, canvas.height));
  return estGris(img) ? versGris(img) : img;
}
