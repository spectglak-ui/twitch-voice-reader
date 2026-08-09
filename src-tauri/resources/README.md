# Ressources bundlées

Ce dossier doit exister physiquement pour que `tauri dev`/`tauri build`
démarrent (Tauri valide au chargement que chaque entrée de
`bundle.resources` dans `tauri.conf.json` pointe vers un chemin existant —
son absence provoque l'erreur `resource path \`resources\piper\` doesn't
exist`).

`piper/` est peuplé automatiquement par `scripts/install-piper.sh` (ou
`.ps1` sous Windows) — voir `docs/GUIDE_COMPILATION.md`, section 2.
Son contenu (binaire Piper, modèles de voix `.onnx`) n'est volontairement
pas versionné (fichiers lourds, spécifiques à la plateforme de build) mais
le dossier lui-même doit rester présent, d'où le `.gitkeep`.
