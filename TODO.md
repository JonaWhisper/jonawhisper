# JonaWhisper — TODO

## Bugs

- [ ] **CGEvent Unicode typing** — Remplacer le clipboard+Cmd+V par `CGEventKeyboardSetUnicodeString` pour taper le texte directement
- [ ] **Tray menu se ferme au premier clic après lancement** — Bug upstream `tray-icon` (manque `acceptsFirstMouse:` sur TrayTarget NSView). Issue ouverte : tray-icon#251. Workaround actuel (menu attaché après build) est le meilleur disponible. Fix = PR upstream ou fork.
- [ ] **Spectre plat/gris\u00e9 pendant l'enregistrement (intermittent)** — Parfois le spectre reste plat et gris\u00e9 pendant qu'on parle, alors que l'audio s'enregistre et transcrit correctement. Le flow audio fonctionne (la pill affiche bien l'\u00e9tat recording, la transcription aboutit). Suspect\u00e9 : race condition dans le flux de donn\u00e9es spectre entre le callback cpal, le mutex `AudioRecorder.spectrum`, la commande `GetSpectrum` et l'\u00e9metteur 30fps. Difficile \u00e0 reproduire \u2014 surveiller et investiguer quand le cas se pr\u00e9sente.

- [x] **T5 correction : répétitions en boucle** — Corrigé : détection de boucle live (6 tokens), n-gram blocking 3→4, repeat penalty 1.2→1.5, max tokens 1.5x→1.2x, seuil longueur 2x→1.5x, `strip_repetition` réécrit pour patterns multi-répétés (sentence + word level).

## Fonctionnalités

- [ ] **Raccourci pour historique rapide** — Touche configurable pour afficher un popup flottant avec les dernières transcriptions. Permet de re-coller rapidement un texte récent sans ouvrir la fenêtre d'historique complète. Style popup léger (comme Spotlight/Alfred), clic ou Enter pour coller l'entrée sélectionnée.
- [ ] **Contrainte bidirectionnelle langue/modèle** — Griser les modèles incompatibles avec la langue sélectionnée, ET griser les langues non supportées par le modèle sélectionné. Les modèles sans `lang_codes` (ex: Whisper) supportent toutes les langues. Concerne ModelsSection.vue + le store engines.
- [ ] **Pipeline prétraitement audio** — VAD + denoising optionnel avant transcription. Voir `docs/AUDIO-PIPELINE.md` pour l'architecture complète. **Important** : le denoising dégrade Whisper (paper "When De-noising Hurts", arXiv:2512.17562) → VAD prioritaire, denoising optionnel et désactivé par défaut.
  - **Phase 2** : Denoising optionnel via nnnoiseless (pure Rust, 85 KB). Toggle dans préférences, désactivé par défaut. Si qualité insuffisante → DeepFilterNet3 (crate `deep_filter`, Rust natif via tract).
  - **Phase 3** : Presets device (gain, noise gate, normalisation par type de micro). Voir `docs/AUDIO-PIPELINE.md` Phase 3.
- [ ] **Restauration après crash** — Sauvegarder l'état de la queue de transcription sur disque (fichiers audio en attente). En cas de crash ou kill pendant une transcription, les fichiers WAV restent dans /tmp mais la queue en mémoire est perdue. Persister la queue permettrait de reprendre automatiquement au relancement. Concerne uniquement la transcription, pas le téléchargement.

## Intégrations Cloud & Modèles

- [ ] **Intégration Deepgram Nova-3** — API propriétaire mais simple (REST, audio brut en body, ~80 lignes Rust). Meilleure qualité sur audio bruité. Voir `docs/CLOUD-INTEGRATION.md`.
- [ ] **Voxtral : vérifier si un crate Rust existe** — Surveiller régulièrement si un crate Rust wrappant voxtral.c est publié sur crates.io (comme whisper-rs pour whisper.cpp). Si oui, migrer du vendoring vers le crate pour simplifier la maintenance.
- [ ] **whisper-rs-sys : retirer le workaround ggml i8mm** — On utilise `CMAKE_TOOLCHAIN_FILE` (`src-tauri/cmake/arm-ggml-fix.cmake`) pour forcer `GGML_NATIVE=OFF` + `GGML_CPU_ARM_ARCH=armv8.2-a+dotprod`, contournant une erreur Clang 16+ (`always_inline 'vmmlaq_s32' requires target feature 'i8mm'`). Le fix est upstream dans whisper.cpp (PR llama.cpp#10890) mais pas encore dans whisper-rs-sys 0.14.1. Checker régulièrement les nouvelles versions sur [Codeberg](https://codeberg.org/whisper-rs/whisper-rs) et crates.io. Quand le fix est inclus : supprimer `arm-ggml-fix.cmake`, retirer `CMAKE_TOOLCHAIN_FILE` de `build.sh`, `ci.yml`, et `release.yml`.
- [x] **Post-processeur rule-based (Harper/GECToR)** — Évalué et écarté (mars 2026). Harper v1.4.1 = EN-only, checks redondants avec finalize()+PCS. GECToR = pas d'ONNX, EN-only, licence non-commerciale, effort élevé. Pas de valeur ajoutée pour un outil bilingue FR/EN.

## Documentation

- [ ] **Guide de setup pour les utilisateurs** — Page `docs/SETUP-GUIDE.md` ou section README expliquant :
  - **Modèles natifs** — quels Whisper/LLM télécharger selon le hardware (RAM, Apple Silicon vs Intel)
  - **Cloud providers** — comment configurer Groq, OpenAI, Cerebras, Gemini (avec les presets)
  - **Serveurs locaux** — pour ceux qui veulent héberger un serveur séparé :
    - **LLM** : Ollama (`brew install ollama && ollama pull qwen3:4b`, URL `http://localhost:11434`) ou LM Studio (GUI, port 1234)
    - **ASR** : whisper.cpp server (`brew install whisper-cpp`, port 8080) ou MLX-Audio (`pip install mlx-audio`, port 8000)
  - Voir `docs/BENCHMARK.md` pour les comparatifs détaillés

## Technique / Infra

- [ ] **Windows support** — Implémenter les vrais bindings (hotkey via `SetWindowsHookEx`, permissions, paste, audio devices)

