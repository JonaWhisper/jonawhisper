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
- [ ] **Moteurs natifs Rust (stratégie "zéro Python")** — Remplacer les subprocess Python/CLI par des bindings Rust natifs. L'objectif est un binaire unique sans dépendances externes, cross-platform (macOS + Windows).
  - **whisper.cpp → [whisper-rs](https://github.com/tazz4843/whisper-rs)** — Binding Rust natif, compilé par `build.rs`. CoreML/Metal sur macOS, CUDA/DirectML sur Windows. Moteur par défaut out-of-the-box. Sélection automatique du meilleur backend (GPU > Neural Engine > CPU). Option avancée dans Settings pour forcer un backend spécifique (Auto / CPU / GPU). Auto par défaut = transparent pour l'utilisateur.
  - **Vosk → [vosk-rs](https://github.com/Bear-03/vosk-rs)** — Binding Rust vers la lib C de Vosk. Léger, cross-platform.
  - **Moonshine → [ort](https://github.com/pykeIO/ort)** (ONNX Runtime pour Rust) — Moonshine utilise des modèles ONNX. Le crate `ort` charge et exécute les modèles ONNX directement depuis Rust. Cross-platform, supporte CPU/GPU/CoreML/DirectML. Pourrait aussi servir pour d'autres modèles ONNX à l'avenir.
  - **Faster Whisper → [ct2rs](https://github.com/jkawamoto/ctranslate2-rs)** — Binding Rust pour CTranslate2 (le moteur derrière Faster Whisper). Moins mature, à évaluer. Si whisper-rs avec CoreML/Metal est suffisamment performant sur Mac, Faster Whisper devient redondant.
  - **MLX Whisper** — MLX est un framework Apple Python/Swift only. Pas de binding Rust. Avec whisper-rs + CoreML on obtient la même accélération Apple Silicon, donc MLX Whisper pourrait être retiré ou gardé comme option Python legacy.
  - **API** — Déjà natif Rust (reqwest). Rien à changer.
  - **Moteurs Python en fallback** — Garder le support subprocess Python en option (venv dédié `~/.whisper-dictate/venv/`) pour les utilisateurs qui préfèrent ou qui ont des modèles spécifiques. Mais ce n'est plus le chemin par défaut.
  - **Téléchargement des modèles** — Utiliser le crate officiel [hf-hub](https://github.com/huggingface/hf-hub) (maintenu par Hugging Face) pour les modèles HF : gestion du cache, progression, reprise de téléchargement, auth tokens. Pour Vosk (hébergé sur alphacephei.com), garder du reqwest classique (URLs directes).
  - **UX cible** : les moteurs Rust natifs sont disponibles immédiatement (juste télécharger le modèle). Les moteurs Python optionnels affichent un bouton "Install" qui gère le venv automatiquement.
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
- [x] **FFI consolidation** — ✅ Module `platform/ffi.rs` partagé (CGEventTapCreate, CGEventGetFlags, CFRunLoop, etc.)
- [ ] **Hotkey static atomics → struct user_info** — Remplacer les 4 static atomics dans hotkey.rs par un struct passé via le user_info du CGEvent callback
- [ ] **Event names centralisés** — Définir les noms d'events Tauri en constantes dans un module `events.rs` (éviter les strings éparpillés)
- [x] **Moonshine shell injection** — ✅ Chemin audio passé via sys.argv
- [x] **activateIgnoringOtherApps deprecated** — ✅ Runtime check : `activate()` sur macOS 14+, fallback `activateIgnoringOtherApps:` sur macOS 13
- [x] **Audit event listeners / doublons** — ✅ Fait. Pas de doublons. Fixes : ajout emit `permission-changed` dans request_permission, suppression emit `download-complete` orphelin, ajout listener `mic-test-stopped` dans Settings.vue
- [ ] **Script de test visuel + screenshots** — Pouvoir lancer des flows de test (pill, settings, etc.) et capturer des screenshots automatiquement pour vérifier le rendu sans intervention manuelle
- [ ] **Windows support** — Implémenter les vrais bindings (hotkey, permissions, paste, audio devices)
