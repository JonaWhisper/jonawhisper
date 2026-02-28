# WhisperDictate — TODO

## Bugs

- [ ] **CGEvent Unicode typing** — Remplacer le clipboard+Cmd+V par `CGEventKeyboardSetUnicodeString` pour taper le texte directement
- [ ] **Tray menu se ferme au premier clic après lancement** — Bug upstream `tray-icon` (manque `acceptsFirstMouse:` sur TrayTarget NSView). Issue ouverte : tray-icon#251. Workaround actuel (menu attaché après build) est le meilleur disponible. Fix = PR upstream ou fork.

## UX / Polish

- [x] **README** — ✅ Fait. Description, build, permissions, engines, usage, tech stack, structure
- [x] **Model Manager : langue en haut** — ✅ Déplacé dans une toolbar fixe en haut du contenu principal
- [x] **Model Manager : layout toolbar en haut (Option A)** — ✅ Langue + bouton "Add Server" en header fixe, liste scrollable en dessous
- [x] **Model Manager : commande d'install visible pour tous les moteurs** — ✅ Affiche la commande même quand le moteur est installé

## Fonctionnalités

- [ ] **Historique des transcriptions (infini)** — Stockage persistant (SQLite ou append-only), UI de consultation/recherche
- [ ] **Restauration après crash** — Sauvegarder l'état de la queue sur disque
- [ ] **Système de raccourcis personnalisés** — "Press to record" pour choisir n'importe quelle combinaison de touches
- [ ] **Système de providers LLM unifié** — Fusionner `engines/ApiServerConfig` (transcription) et `LlmConfig` (cleanup) en un système unique de providers. Chaque provider déclare ses capacités : transcription audio→texte, nettoyage texte→texte, ou les deux. Config serveur partagée (URL, clé API, modèle). Support local (Ollama, llama.cpp) en plus des API distantes.
  - **Providers cloud pré-configurés** — OpenAI, Anthropic, Gemini : URL et liste de modèles prédéfinis (dropdown), champs non-modifiables. Champs custom (URL, modèle libre) uniquement pour serveurs locaux/custom.
  - **Formulaire provider unifié** — Un seul composant Vue partagé entre Model Manager et Settings (éviter la duplication du formulaire d'ajout de serveur)
- [ ] **Installation intégrée des moteurs** — Supprimer la friction CLI, installer les moteurs depuis l'app directement. Approche hybride :
  - **whisper.cpp intégré via whisper-rs** — Lier whisper.cpp directement dans le binaire Rust via le crate [whisper-rs](https://github.com/tazz4843/whisper-rs). Compilé depuis les sources par `build.rs`, zéro dépendance externe. Supporte CoreML/Metal pour l'accélération Apple Silicon. Appel de fonction direct (pas de subprocess). C'est le moteur par défaut, ça marche out-of-the-box.
  - **Venv Python dédié** — Pour les moteurs Python (Faster Whisper, MLX Whisper, Vosk, Moonshine) : l'app crée et gère `~/.whisper-dictate/venv/`, installe les packages dedans. Bouton "Install" dans le Model Manager avec barre de progression. Nécessite un Python système détecté automatiquement (`python3` / `python`).
  - **API = rien à installer** — OpenAI API et serveurs custom restent en config pure (URL + clé).
  - **UX cible** : dans le Model Manager, chaque moteur non installé affiche un bouton "Install" au lieu de la commande CLI. Le bouton lance l'install en background, affiche la progression, et rafraîchit le statut une fois terminé.
- [ ] **Descriptions des moteurs dans le Model Manager** — Ajouter une description/sous-titre pour chaque moteur expliquant sa spécificité :
  - **Whisper** (whisper.cpp) — C++, rapide sur CPU, le plus léger. Moteur par défaut.
  - **Faster Whisper** (CTranslate2) — Optimisé GPU via CTranslate2, ~4x plus rapide que le Whisper original. Idéal si GPU NVIDIA disponible.
  - **MLX Whisper** — Optimisé Apple Silicon (Neural Engine / GPU unifié via MLX). Le plus performant sur Mac M1/M2/M3/M4.
  - **Vosk** — Léger, offline, fonctionne bien sur CPU faible. Modèles petits. Moins précis que Whisper.
  - **Moonshine** — Ultra-léger et rapide, anglais uniquement. Idéal pour de la dictée rapide en anglais.
  - **OpenAI API** — Transcription cloud via API OpenAI-compatible. Pas de ressources locales nécessaires, mais nécessite une connexion internet et une clé API.
  - Afficher cette description dans la sidebar du Model Manager sous le nom du moteur, et/ou dans le header du panneau principal quand le moteur est sélectionné.
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
