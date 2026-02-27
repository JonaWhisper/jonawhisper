# WhisperDictate — TODO

## Bugs

- [ ] **CGEvent Unicode typing** — Remplacer le clipboard+Cmd+V par `CGEventKeyboardSetUnicodeString` pour taper le texte directement
- [ ] **Tray menu se ferme au premier clic après lancement** — Le tout premier clic sur l'icône tray après le démarrage de l'app ferme le menu au lieu de l'ouvrir

## UX / Polish

- [ ] **Settings window : taille adaptative** — Hauteur prioritaire, colonnes si besoin

## Fonctionnalités

- [ ] **Historique des transcriptions (infini)** — Stockage persistant (SQLite ou append-only), UI de consultation/recherche
- [ ] **Restauration après crash** — Sauvegarder l'état de la queue sur disque
- [ ] **Système de raccourcis personnalisés** — "Press to record" pour choisir n'importe quelle combinaison de touches
- [ ] **LLM post-processing : modèle local** — llama.cpp ou subprocess en plus des API distantes
- [ ] **Presets audio par type de device** — Gain, noise gate, normalisation selon micro intégré/AirPods/casque/USB

## Technique / Infra

- [ ] **build.sh double exécution** — Le script semble s'exécuter deux fois, investiguer pourquoi
- [ ] **Audit des approches et patterns** — Lister toutes les approches utilisées (channels, mutex, FFI, async, spawning, etc.), comparer aux best practices Rust/Tauri, identifier les incohérences et améliorations possibles
- [ ] **Script de test visuel + screenshots** — Pouvoir lancer des flows de test (pill, settings, etc.) et capturer des screenshots automatiquement pour vérifier le rendu sans intervention manuelle
- [ ] **Windows support** — Implémenter les vrais bindings (hotkey, permissions, paste, audio devices)
