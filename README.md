# Traitement d'image

Implémentation des opérations de traitement d'image vues en cours (M2 SIG).
La liste complète est dans [`liste-fonctionnalites.txt`](liste-fonctionnalites.txt).

## Organisation

Chaque membre choisit son langage (C ou Rust) :

```
c/       code en C
rust/    code en Rust
images/  images de test partagées (PGM / PPM)
```

Format d'entrée/sortie commun : **PGM** (niveaux de gris) et **PPM** (couleur RGB),
pour que tout le monde puisse tester sur les mêmes images.

## Répartition du travail

| Bloc | Responsable | Langage | État |
|------|-------------|---------|------|
| Entrées / sorties PGM-PPM | | | |
| Introduction (canaux, gris, profil, quantification) | | | |
| Statistiques et histogramme | | | |
| Amélioration du contraste | | | |
| Opérations sur images | | | |
| Interpolation | | | |
| Convolution et filtrage spatial | | | |
| Fourier et filtrage spectral | | | |
| Détection de contours | | | |
| Segmentation | | | |
| Images binaires | | | |
| Morphologie mathématique | | | |

## Travailler à plusieurs

1. Créer une branche par bloc : `git switch -c contours`
2. Commit et push sur sa branche
3. Ouvrir une pull request vers `main`
