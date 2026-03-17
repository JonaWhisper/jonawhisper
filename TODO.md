# JonaWhisper — TODO

## Bugs

- [ ] **CGEvent Unicode typing** — Remplacer le clipboard+Cmd+V par `CGEventKeyboardSetUnicodeString` pour taper le texte directement
- [ ] **Tray menu se ferme au premier clic après lancement** — Bug upstream `tray-icon` (manque `acceptsFirstMouse:` sur TrayTarget NSView). Issue ouverte : tray-icon#251. Workaround actuel (menu attaché après build) est le meilleur disponible. Fix = PR upstream ou fork.
- [ ] **Spectre plat/grisé pendant l'enregistrement (intermittent)** — Partiellement résolu. Fix Mutex→AtomicSpectrum (commit 4c05986) a corrigé la contention, mais le bug persiste. Diagnostic amélioré (PR #34 + #35) : seuil visuel VISUAL_FLAT_THRESHOLD=0.12, logging périodique dans spectrum emitter + pill render, reset smoothed on Recording enter. Attendre les prochains logs pour identifier la cause racine.
- [ ] **Latence aléatoire de transcription** — La première transcription après lancement prend ~5s (chargement Parakeet ONNX, normal). Mais parfois des transcriptions ultérieures sont aussi lentes sans raison apparente. Diagnostic ajouté : log `ContextMap: loading context` (avec durée) et `Transcription total` + RSS dans pipeline.rs.

## Fonctionnalités

- [ ] **Raccourci pour historique rapide** — Touche configurable pour afficher un popup flottant avec les dernières transcriptions. Permet de re-coller rapidement un texte récent sans ouvrir la fenêtre d'historique complète. Style popup léger (comme Spotlight/Alfred), clic ou Enter pour coller l'entrée sélectionnée.
- [ ] **Contrainte bidirectionnelle langue/modèle** — Griser les modèles incompatibles avec la langue sélectionnée, ET griser les langues non supportées par le modèle sélectionné. Les modèles sans `lang_codes` (ex: Whisper) supportent toutes les langues. Concerne ModelsSection.vue + le store engines.
- [ ] **Denoising optionnel** — Pipeline hybride (dénoisé pour VAD boundaries, original pour ASR). Voir `docs/AUDIO-PIPELINE.md` Phase 2. **Important** : le denoising dégrade Whisper si envoyé directement (paper "When De-noising Hurts", arXiv:2512.17562) → désactivé par défaut.
  - **Phase 2** : Denoising optionnel via nnnoiseless (pure Rust, 85 KB). Toggle dans préférences, désactivé par défaut. Si qualité insuffisante → DeepFilterNet3 (crate `deep_filter`, Rust natif via tract).
  - **Phase 3** : Presets device (gain, noise gate, normalisation par type de micro). Voir `docs/AUDIO-PIPELINE.md` Phase 3.
- [ ] **Restauration après crash** — Sauvegarder l'état de la queue de transcription sur disque (fichiers audio en attente). En cas de crash ou kill pendant une transcription, les fichiers WAV restent dans /tmp mais la queue en mémoire est perdue. Persister la queue permettrait de reprendre automatiquement au relancement.

## Intégrations Cloud & Modèles

- [ ] **Intégration Deepgram Nova-3** — API propriétaire mais simple (REST, audio brut en body, ~80 lignes Rust). Meilleure qualité sur audio bruité. Voir `docs/CLOUD-PROVIDERS.md`.
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

## Axes d'amélioration pipeline texte

- [ ] **Filtrage hallucinations par log-probabilité** — Utiliser les token log-probs et le compression ratio de Whisper pour détecter les hallucinations de manière plus robuste que les listes de phrases. Papers : "Whispering LLaMA" (2023), "Hallucination detection in neural ASR" (2024). Haut impact, effort modéré (Whisper expose déjà ces métriques, les autres engines non).
- [ ] **Filtrage phonétique des candidats SymSpell** — Intégrer un score de similarité phonétique (Soundex/Metaphone) pour filtrer les faux positifs SymSpell. Les erreurs ASR sont phonétiquement proches de la cible — SymSpell ne le sait pas et propose parfois des corrections aberrantes. Effort modéré (crate `rphonetic` déjà évalué dans GEC-RESEARCH.md).
- [ ] **Passe unique LLM remplaçant spell+punct+GEC** — À terme, un seul appel LLM local (Qwen3 4B ou équivalent) pourrait remplacer la chaîne SymSpell → PCS → T5. Avantage : cohérence globale de la correction, moins de passes, gestion du contexte. Risque : latence (~1s), hallucinations LLM. Évaluer sur un benchmark FR/EN avant migration. Paper : "LLM-based post-editing for ASR" (2024).
- [ ] **Ponctuation domain-adapted** — Fine-tuner PCS ou BERT sur un corpus ASR (transcriptions orales avec ponctuation de référence) plutôt que du texte écrit. Les modèles actuels sont entraînés sur texte formel, pas sur de l'oral transcrit. Impact modéré, effort élevé (besoin de données annotées).

## Audit / Qualité

- [ ] **Persistance du contexte chargé par modèle** — Étudier si on peut persister le contexte (poids chargés en mémoire, état ONNX/CoreML, caches) de chaque modèle entre les lancements de l'app, pour éviter le cold-start à chaque démarrage. Actuellement le `ContextMap` recharge tout à la première utilisation (~5s pour Parakeet). Pistes : sérialisation des sessions ort, mmap des poids, cache CoreML compilé.
- [ ] **Contexte conversationnel unifié entre modèles** — Maintenir un contexte grammatical/sémantique partagé entre les transcriptions successives. Actuellement chaque transcription est indépendante — le modèle ne sait pas ce qui a été dit avant. Pistes : (1) prompt contextuel avec les N dernières phrases pour les modèles qui supportent le conditioning (Whisper `initial_prompt`, Qwen, LLM cloud), (2) contexte partagé pour le post-processing (spell-check, ponctuation, correction) — le correcteur pourrait mieux désambiguïser avec le contexte précédent, (3) unifier ce contexte entre ASR et cleanup pour une cohérence globale.

## Technique / Infra

- [ ] **Windows support** — Implémenter les vrais bindings (hotkey via `SetWindowsHookEx`, permissions, paste, audio devices)
