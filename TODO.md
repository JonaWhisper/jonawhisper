# JonaWhisper — TODO

## Bugs

- [ ] **CGEvent Unicode typing** — Remplacer le clipboard+Cmd+V par `CGEventKeyboardSetUnicodeString` pour taper le texte directement
- [ ] **Tray menu se ferme au premier clic après lancement** — Bug upstream `tray-icon` (manque `acceptsFirstMouse:` sur TrayTarget NSView). Issue ouverte : tray-icon#251. Workaround actuel (menu attaché après build) est le meilleur disponible. Fix = PR upstream ou fork.
- [ ] **Spectre plat/grisé pendant l'enregistrement (intermittent)** — Partiellement résolu. Fix Mutex→AtomicSpectrum (commit 4c05986) a corrigé la contention, mais le bug persiste : spectre visuellement plat sur toute la durée d'un enregistrement, et reste plat sur toutes les dictées suivantes. Pas de warning dans les logs → les valeurs FFT sont au-dessus du seuil 0.001 mais en-dessous du seuil visuel (~0.12). **Diagnostic amélioré** (PR #34) : seuil visuel VISUAL_FLAT_THRESHOLD=0.12, logging périodique des valeurs brutes/lissées, fft_buffer contention promu en warn. Attendre les prochains logs pour identifier la cause racine.
- [x] **Setup wizard ne s'affiche pas après relancement (permissions déjà accordées)** — Résolu (PR #37). Flag `setup_completed` persisté dans Preferences. Le wizard s'affiche tant que le flag est false, saute au step 2 si les permissions sont déjà OK.
- [x] **Check Input Monitoring trop permissif** — Résolu (PR #35). Masque élargi à keyDown+keyUp+flagsChanged dans `macos.rs`.
- [ ] **Latence aléatoire de transcription** — La première transcription après lancement prend ~5s (chargement Parakeet ONNX, normal). Mais parfois des transcriptions ultérieures sont aussi lentes sans raison apparente, alors que la plupart sont à 0.1-0.2s. **Diagnostic ajouté** : log `ContextMap: loading context` (avec durée) et `Transcription total` dans pipeline.rs. Hypothèses : rechargement inattendu du modèle (ContextMap éviction), CoreML recompilation, ou contention de lock. À surveiller avec `log show --predicate 'process == "jona-whisper"' --last 5m | grep -E "ContextMap|Transcription total"`.
- [x] **SymSpell dégrade la transcription** — Résolu. KenLM C++ vendoré (`jona-engine-lm`) avec reranking trigram contextuel dans `symspell_correct.rs`. 9 modèles de langue entraînés sur Wikipedia (pruned + quantized 8-bit, 50-100 MB/lang), hébergés sur HuggingFace `JonaWhisper/kenlm-models`. French guards ajoutés (pluriel, apostrophe/élision). Dynamic max_distance (1 pour mots <6 chars, 2 pour les plus longs). Chargement deadlock-free (load hors du mutex).


## Fonctionnalités

- [ ] **Raccourci pour historique rapide** — Touche configurable pour afficher un popup flottant avec les dernières transcriptions. Permet de re-coller rapidement un texte récent sans ouvrir la fenêtre d'historique complète. Style popup léger (comme Spotlight/Alfred), clic ou Enter pour coller l'entrée sélectionnée.
- [ ] **Contrainte bidirectionnelle langue/modèle** — Griser les modèles incompatibles avec la langue sélectionnée, ET griser les langues non supportées par le modèle sélectionné. Les modèles sans `lang_codes` (ex: Whisper) supportent toutes les langues. Concerne ModelsSection.vue + le store engines.
- [x] **VAD (Silero v6.2)** — Détection de parole avant transcription. Discard silence, trimming début/fin, toggle `vad_enabled`. Voir `docs/AUDIO-PIPELINE.md` Phase 1.
- [ ] **Denoising optionnel** — Pipeline hybride (dénoisé pour VAD boundaries, original pour ASR). Voir `docs/AUDIO-PIPELINE.md` Phase 2. **Important** : le denoising dégrade Whisper si envoyé directement (paper "When De-noising Hurts", arXiv:2512.17562) → désactivé par défaut.
  - **Phase 2** : Denoising optionnel via nnnoiseless (pure Rust, 85 KB). Toggle dans préférences, désactivé par défaut. Si qualité insuffisante → DeepFilterNet3 (crate `deep_filter`, Rust natif via tract).
  - **Phase 3** : Presets device (gain, noise gate, normalisation par type de micro). Voir `docs/AUDIO-PIPELINE.md` Phase 3.
- [ ] **Restauration après crash** — Sauvegarder l'état de la queue de transcription sur disque (fichiers audio en attente). En cas de crash ou kill pendant une transcription, les fichiers WAV restent dans /tmp mais la queue en mémoire est perdue. Persister la queue permettrait de reprendre automatiquement au relancement. Concerne uniquement la transcription, pas le téléchargement.

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
- [x] **Dictionnaire utilisateur / biasing contextuel** — Implémenté. Panneau dédié "Dictionnaire" dans la sidebar. Deux types d'entrées : mots protégés (conservés tels quels par le spell-check) et mappings de remplacement (`pattern=replacement`, appliqués pendant l'ITN). Fichier `user_dict.txt` dans le répertoire de données de l'app.
- [ ] **Filtrage phonétique des candidats SymSpell** — Intégrer un score de similarité phonétique (Soundex/Metaphone) pour filtrer les faux positifs SymSpell. Les erreurs ASR sont phonétiquement proches de la cible — SymSpell ne le sait pas et propose parfois des corrections aberrantes. Effort modéré (crate `rphonetic` déjà évalué dans GEC-RESEARCH.md).
- [ ] **Passe unique LLM remplaçant spell+punct+GEC** — À terme, un seul appel LLM local (Qwen3 4B ou équivalent) pourrait remplacer la chaîne SymSpell → PCS → T5. Avantage : cohérence globale de la correction, moins de passes, gestion du contexte. Risque : latence (~1s), hallucinations LLM. Évaluer sur un benchmark FR/EN avant migration. Paper : "LLM-based post-editing for ASR" (2024).
- [ ] **Ponctuation domain-adapted** — Fine-tuner PCS ou BERT sur un corpus ASR (transcriptions orales avec ponctuation de référence) plutôt que du texte écrit. Les modèles actuels sont entraînés sur texte formel, pas sur de l'oral transcrit. Impact modéré, effort élevé (besoin de données annotées).
- [x] **Correction sélective guidée par confiance** — Implémenté dans `symspell_correct.rs` : les mots avec un score ASR > 0.85 sont ignorés par le spell-check (`CONFIDENCE_SKIP_THRESHOLD`). Parakeet et Canary fournissent les scores, Whisper non (tous les mots sont corrigés). La correction T5 ne filtre pas encore par confiance (piste d'amélioration).

## Audit / Qualité

- [ ] **Persistance du contexte chargé par modèle** — Étudier si on peut persister le contexte (poids chargés en mémoire, état ONNX/CoreML, caches) de chaque modèle entre les lancements de l'app, pour éviter le cold-start à chaque démarrage. Actuellement le `ContextMap` recharge tout à la première utilisation (~5s pour Parakeet). Pistes : sérialisation des sessions ort, mmap des poids, cache CoreML compilé.
- [ ] **Contexte conversationnel unifié entre modèles** — Maintenir un contexte grammatical/sémantique partagé entre les transcriptions successives. Actuellement chaque transcription est indépendante — le modèle ne sait pas ce qui a été dit avant. Pistes : (1) prompt contextuel avec les N dernières phrases pour les modèles qui supportent le conditioning (Whisper `initial_prompt`, Qwen, LLM cloud), (2) contexte partagé pour le post-processing (spell-check, ponctuation, correction) — le correcteur pourrait mieux désambiguïser avec le contexte précédent, (3) unifier ce contexte entre ASR et cleanup pour une cohérence globale. Impact : meilleure ponctuation inter-phrases, correction contextuelle, désambiguïsation des homophones.

## Mémoire / Performance

- [x] **Libération mémoire ORT après longues dictées** — Résolu (PR #35). Auto-release des contextes engine après transcriptions >30s via `maybe_auto_release` dans pipeline.rs. Toggle `auto_release_memory` dans les préférences (défaut: activé). Model_id capturé avant le spawn pour invalider le bon engine.
- [x] **Section Diagnostic dans les settings** — Résolu (PR #35). `DiagnosticSection.vue` avec RSS en temps réel (cross-platform: macOS/Linux/Windows), contextes chargés dans ContextMap, toggle auto-release. Struct typé `MemoryInfo` dans jona-types.
- [x] **Logging mémoire après chaque transcription** — Résolu (PR #35). `log::info!("Transcription total: {:.1}s | RSS: {}")` dans pipeline.rs après chaque transcription.

## Technique / Infra

- [x] **Refonte du système de logging** — Résolu. Fichier de log persistant (`~/Library/Application Support/JonaWhisper/logs/`, rotation par lancement, rétention configurable). Niveaux de log configurables dans les préférences (`log_level`). Logs enrichis dans : pill rendering (mode transitions, flat detection, smoothed values), spectrum pipeline (diagnostic périodique, seuil visuel, recovery), audio.rs (fft_buffer contention en warn), pipeline.rs (RSS après chaque transcription). Reste à faire éventuellement : filtrage par module (nice-to-have).
- [ ] **Windows support** — Implémenter les vrais bindings (hotkey via `SetWindowsHookEx`, permissions, paste, audio devices)
