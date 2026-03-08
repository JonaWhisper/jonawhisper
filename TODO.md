# JonaWhisper — TODO

## Bugs

- [ ] **CGEvent Unicode typing** — Remplacer le clipboard+Cmd+V par `CGEventKeyboardSetUnicodeString` pour taper le texte directement
- [ ] **Tray menu se ferme au premier clic après lancement** — Bug upstream `tray-icon` (manque `acceptsFirstMouse:` sur TrayTarget NSView). Issue ouverte : tray-icon#251. Workaround actuel (menu attaché après build) est le meilleur disponible. Fix = PR upstream ou fork.
- [ ] **Spectre plat/grisé pendant l'enregistrement (intermittent)** — Parfois le spectre reste plat et grisé pendant qu'on parle, alors que l'audio s'enregistre et transcrit correctement. Le flow audio fonctionne (la pill affiche bien l'état recording, la transcription aboutit). Suspecté : race condition dans le flux de données spectre entre le callback cpal, le mutex `AudioRecorder.spectrum`, la commande `GetSpectrum` et l'émetteur 30fps. Difficile à reproduire. **Diagnostic ajouté** : log `Spectrum flat while recording` + queue depth dans `spawn_spectrum_emitter`.
- [ ] **Latence aléatoire de transcription** — La première transcription après lancement prend ~5s (chargement Parakeet ONNX, normal). Mais parfois des transcriptions ultérieures sont aussi lentes sans raison apparente, alors que la plupart sont à 0.1-0.2s. **Diagnostic ajouté** : log `ContextMap: loading context` (avec durée) et `Transcription total` dans pipeline.rs. Hypothèses : rechargement inattendu du modèle (ContextMap éviction), CoreML recompilation, ou contention de lock. À surveiller avec `log show --predicate 'process == "jona-whisper"' --last 5m | grep -E "ContextMap|Transcription total"`.
- [ ] **SymSpell dégrade la transcription** — **En cours** : crate `jona-engine-lm` créé avec KenLM C++ vendoré + intégration dans `symspell_correct.rs` (reranking trigram contextuel). Reste : entraîner les modèles trigram FR/EN sur Wikipedia (pruned + quantized), uploader sur HuggingFace `JonaWhisper/kenlm-models`, mettre à jour `manifest.json` avec les tailles réelles.


## Fonctionnalités

- [ ] **Raccourci pour historique rapide** — Touche configurable pour afficher un popup flottant avec les dernières transcriptions. Permet de re-coller rapidement un texte récent sans ouvrir la fenêtre d'historique complète. Style popup léger (comme Spotlight/Alfred), clic ou Enter pour coller l'entrée sélectionnée.
- [ ] **Contrainte bidirectionnelle langue/modèle** — Griser les modèles incompatibles avec la langue sélectionnée, ET griser les langues non supportées par le modèle sélectionné. Les modèles sans `lang_codes` (ex: Whisper) supportent toutes les langues. Concerne ModelsSection.vue + le store engines.
- [x] **VAD (Silero v6.2)** — Détection de parole avant transcription. Discard silence, trimming début/fin, toggle `vad_enabled`. Voir `docs/AUDIO-PIPELINE.md` Phase 1.
- [ ] **Denoising optionnel** — Pipeline hybride (dénoisé pour VAD boundaries, original pour ASR). Voir `docs/AUDIO-PIPELINE.md` Phase 2. **Important** : le denoising dégrade Whisper si envoyé directement (paper "When De-noising Hurts", arXiv:2512.17562) → désactivé par défaut.
  - **Phase 2** : Denoising optionnel via nnnoiseless (pure Rust, 85 KB). Toggle dans préférences, désactivé par défaut. Si qualité insuffisante → DeepFilterNet3 (crate `deep_filter`, Rust natif via tract).
  - **Phase 3** : Presets device (gain, noise gate, normalisation par type de micro). Voir `docs/AUDIO-PIPELINE.md` Phase 3.
- [ ] **Restauration après crash** — Sauvegarder l'état de la queue de transcription sur disque (fichiers audio en attente). En cas de crash ou kill pendant une transcription, les fichiers WAV restent dans /tmp mais la queue en mémoire est perdue. Persister la queue permettrait de reprendre automatiquement au relancement. Concerne uniquement la transcription, pas le téléchargement.

## Intégrations Cloud & Modèles

- [ ] **Intégration Deepgram Nova-3** — API propriétaire mais simple (REST, audio brut en body, ~80 lignes Rust). Meilleure qualité sur audio bruité. Voir `docs/CLOUD-INTEGRATION.md`.
- [ ] **Voxtral : vérifier si un crate Rust existe** — Surveiller régulièrement si un crate Rust wrappant voxtral.c est publié sur crates.io (comme whisper-rs pour whisper.cpp). Si oui, migrer du vendoring vers le crate pour simplifier la maintenance.
- [ ] **whisper-rs-sys : retirer le workaround ggml i8mm** — On utilise `CMAKE_TOOLCHAIN_FILE` (`src-tauri/cmake/arm-ggml-fix.cmake`) pour forcer `GGML_NATIVE=OFF` + `GGML_CPU_ARM_ARCH=armv8.2-a+dotprod`, contournant une erreur Clang 16+ (`always_inline 'vmmlaq_s32' requires target feature 'i8mm'`). Le fix est upstream dans whisper.cpp (PR llama.cpp#10890) mais pas encore dans whisper-rs-sys 0.14.1. Checker régulièrement les nouvelles versions sur [Codeberg](https://codeberg.org/whisper-rs/whisper-rs) et crates.io. Quand le fix est inclus : supprimer `arm-ggml-fix.cmake`, retirer `CMAKE_TOOLCHAIN_FILE` de `build.sh`, `ci.yml`, et `release.yml`.
## Documentation

- [ ] **Guide de setup pour les utilisateurs** — Page `docs/SETUP-GUIDE.md` ou section README expliquant :
  - **Modèles natifs** — quels Whisper/LLM télécharger selon le hardware (RAM, Apple Silicon vs Intel)
  - **Cloud providers** — comment configurer Groq, OpenAI, Cerebras, Gemini (avec les presets)
  - **Serveurs locaux** — pour ceux qui veulent héberger un serveur séparé :
    - **LLM** : Ollama (`brew install ollama && ollama pull qwen3:4b`, URL `http://localhost:11434`) ou LM Studio (GUI, port 1234)
    - **ASR** : whisper.cpp server (`brew install whisper-cpp`, port 8080) ou MLX-Audio (`pip install mlx-audio`, port 8000)
  - Voir `docs/BENCHMARK.md` pour les comparatifs détaillés

## Technique / Infra

- [ ] **Refonte du système de logging** — Le logging actuel est insuffisant pour diagnostiquer les bugs en production (pill grisé, latence aléatoire). Problèmes identifiés :
  - Pas de fichier de log persistant (uniquement `os_log` via `log show`, éphémère et filtrage laborieux)
  - Logs critiques manquants dans les chemins chauds (pill rendering, spectrum pipeline, lock contention)
  - Pas de niveaux de log configurables par module (tout ou rien)
  - **Plan** : ajouter un fichier de log rotatif (`~/Library/Logs/JonaWhisper/`) en plus de `os_log`, avec filtrage par module configurable dans les préférences. Enrichir les logs dans : `ui/pill.rs` (état rendu, frames, transitions), `recording/threads.rs` (spectrum pipeline), `audio.rs` (try_lock contention), `cleanup/` (durées par étape). Format structuré avec timestamps précis pour corréler les événements.
- [ ] **Windows support** — Implémenter les vrais bindings (hotkey via `SetWindowsHookEx`, permissions, paste, audio devices)
