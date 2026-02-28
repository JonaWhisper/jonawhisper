# WhisperDictate — TODO

## Bugs

- [ ] **CGEvent Unicode typing** — Remplacer le clipboard+Cmd+V par `CGEventKeyboardSetUnicodeString` pour taper le texte directement
- [ ] **Tray menu se ferme au premier clic après lancement** — Bug upstream `tray-icon` (manque `acceptsFirstMouse:` sur TrayTarget NSView). Issue ouverte : tray-icon#251. Workaround actuel (menu attaché après build) est le meilleur disponible. Fix = PR upstream ou fork.

## UX / Polish

- [ ] **README** — Écrire un README propre pour le repo (description, screenshots, install, usage, build)
- [ ] **Model Manager : langue en haut** — Déplacer la sélection de langue en haut de la fenêtre (actuellement en bas, se perd quand la liste de modèles grandit)
- [ ] **Model Manager : layout toolbar en haut (Option A)** — Langue + bouton "Add Server" en header fixe, liste de modèles scrollable en dessous
- [ ] **Model Manager : commande d'install visible pour tous les moteurs** — Afficher la commande d'installation même pour les moteurs déjà installés (ex: `brew install whisper-cpp`), pas seulement quand ils sont absents

## Fonctionnalités

- [ ] **Historique des transcriptions (infini)** — Stockage persistant (SQLite ou append-only), UI de consultation/recherche
- [ ] **Restauration après crash** — Sauvegarder l'état de la queue sur disque
- [ ] **Système de raccourcis personnalisés** — "Press to record" pour choisir n'importe quelle combinaison de touches
- [ ] **Système de providers LLM unifié** — Fusionner `engines/ApiServerConfig` (transcription) et `LlmConfig` (cleanup) en un système unique de providers. Chaque provider déclare ses capacités : transcription audio→texte, nettoyage texte→texte, ou les deux. Config serveur partagée (URL, clé API, modèle). Support local (Ollama, llama.cpp) en plus des API distantes.
  - **Providers cloud pré-configurés** — OpenAI, Anthropic, Gemini : URL et liste de modèles prédéfinis (dropdown), champs non-modifiables. Champs custom (URL, modèle libre) uniquement pour serveurs locaux/custom.
  - **Formulaire provider unifié** — Un seul composant Vue partagé entre Model Manager et Settings (éviter la duplication du formulaire d'ajout de serveur)
- [ ] **Presets audio par type de device** — Gain, noise gate, normalisation selon micro intégré/AirPods/casque/USB

## Technique / Infra

- [ ] **CI/CD GitHub Actions** — Pipeline automatique : bump de version → tag → build macOS (.app/.dmg) + Windows → release GitHub avec changelog auto-généré
- [ ] **CHANGELOG.md** — Fichier changelog versionné dans le repo, en plus du changelog dans les releases GitHub
- [x] **Audit des approches et patterns** — ✅ Fait. Fixes appliqués : cache FFT, temp files, shared HTTP clients, lock ordering, Mutex inutiles. Reste à faire ci-dessous.
- [ ] **FFI consolidation** — Regrouper les déclarations `extern "C"` CoreGraphics/CoreFoundation dupliquées (hotkey.rs + macos.rs) dans un module `ffi.rs` partagé
- [ ] **Hotkey static atomics → struct user_info** — Remplacer les 4 static atomics dans hotkey.rs par un struct passé via le user_info du CGEvent callback
- [ ] **Event names centralisés** — Définir les noms d'events Tauri en constantes dans un module `events.rs` (éviter les strings éparpillés)
- [ ] **Moonshine shell injection** — Passer le chemin audio via sys.argv au lieu de format! dans la string Python
- [ ] **activateIgnoringOtherApps deprecated** — Remplacer par `activate()` (macOS 14+)
- [x] **Audit event listeners / doublons** — ✅ Fait. Pas de doublons. Fixes : ajout emit `permission-changed` dans request_permission, suppression emit `download-complete` orphelin, ajout listener `mic-test-stopped` dans Settings.vue
- [ ] **Script de test visuel + screenshots** — Pouvoir lancer des flows de test (pill, settings, etc.) et capturer des screenshots automatiquement pour vérifier le rendu sans intervention manuelle
- [ ] **Windows support** — Implémenter les vrais bindings (hotkey, permissions, paste, audio devices)
