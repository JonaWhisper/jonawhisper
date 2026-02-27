# WhisperDictate — TODO

## Bugs

- [ ] **CGEvent Unicode typing** — Remplacer le clipboard+Cmd+V par `CGEventKeyboardSetUnicodeString` pour taper le texte directement
- [ ] **Tray menu se ferme au premier clic après lancement** — Bug upstream `tray-icon` (manque `acceptsFirstMouse:` sur TrayTarget NSView). Issue ouverte : tray-icon#251. Workaround actuel (menu attaché après build) est le meilleur disponible. Fix = PR upstream ou fork.

## UX / Polish

- [ ] **Settings window : taille adaptative** — Hauteur prioritaire, colonnes si besoin
- [ ] **README** — Écrire un README propre pour le repo (description, screenshots, install, usage, build)

## Fonctionnalités

- [ ] **Historique des transcriptions (infini)** — Stockage persistant (SQLite ou append-only), UI de consultation/recherche
- [ ] **Restauration après crash** — Sauvegarder l'état de la queue sur disque
- [ ] **Système de raccourcis personnalisés** — "Press to record" pour choisir n'importe quelle combinaison de touches
- [ ] **LLM post-processing : modèle local** — llama.cpp ou subprocess en plus des API distantes
- [ ] **Presets audio par type de device** — Gain, noise gate, normalisation selon micro intégré/AirPods/casque/USB

## Technique / Infra

- [ ] **CI/CD GitHub Actions** — Pipeline automatique : bump de version → tag → build macOS (.app/.dmg) + Windows → release GitHub avec changelog auto-généré
- [ ] **build.sh double exécution** — Le script semble s'exécuter deux fois, investiguer pourquoi
- [ ] **Audit des approches et patterns** — Lister toutes les approches utilisées (channels, mutex, FFI, async, spawning, etc.), comparer aux best practices Rust/Tauri, identifier doublons (ex: config+code), incohérences et améliorations possibles
- [ ] **Audit event listeners / doublons** — Vérifier qu'on n'a pas de doubles `on`/`listen` à plusieurs endroits (config Tauri + code Rust), comme le problème qu'on avait eu avec le tray icon
- [ ] **Script de test visuel + screenshots** — Pouvoir lancer des flows de test (pill, settings, etc.) et capturer des screenshots automatiquement pour vérifier le rendu sans intervention manuelle
- [ ] **Windows support** — Implémenter les vrais bindings (hotkey, permissions, paste, audio devices)
