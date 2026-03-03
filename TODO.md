# WhisperDictate — TODO

## Bugs

- [ ] **CGEvent Unicode typing** — Remplacer le clipboard+Cmd+V par `CGEventKeyboardSetUnicodeString` pour taper le texte directement
- [ ] **Tray menu se ferme au premier clic après lancement** — Bug upstream `tray-icon` (manque `acceptsFirstMouse:` sur TrayTarget NSView). Issue ouverte : tray-icon#251. Workaround actuel (menu attaché après build) est le meilleur disponible. Fix = PR upstream ou fork.

## Fonctionnalités

- [x] **Raccourcis multi-touches** — **Done.** Capture accumule jusqu'à 4 touches, finalise au premier relâchement. Packed atomics lock-free (`4×u16` dans `AtomicU64`) pour le callback CGEvent. `peak_modifiers` (OR cumulatif) conserve les modifiers même s'ils sont relâchés avant les touches régulières. Backward compat (ancien JSON `key_code` singulier + legacy strings). Voir `platform/hotkey.rs`, `utils/shortcut.ts`.
- [x] **Section Permissions dans Settings** — **Done.** Affiche le statut des 3 permissions macOS (Microphone, Accessibilité, Surveillance des entrées) avec boutons Grant et badge "Autorisé". Polling 1.5s. Voir `sections/PermissionsSection.vue`.
- [x] **Panel unifié** — **Done.** Remplacé les vues séparées (Settings, History, ModelManager) par un Panel unique avec sidebar + 9 sections (Recents, Models, Transcription, Processing, Shortcuts, Microphone, Providers, Permissions, General). Voir `views/Panel.vue`, `sections/`.
- [ ] **Raccourci pour historique rapide** — Touche configurable pour afficher un popup flottant avec les dernières transcriptions. Permet de re-coller rapidement un texte récent sans ouvrir la fenêtre d'historique complète. Style popup léger (comme Spotlight/Alfred), clic ou Enter pour coller l'entrée sélectionnée.
- [ ] **Pipeline prétraitement audio** — VAD + denoising optionnel avant transcription. Voir `docs/AUDIO-PIPELINE.md` pour l'architecture complète. **Important** : le denoising dégrade Whisper (paper "When De-noising Hurts", arXiv:2512.17562) → VAD prioritaire, denoising optionnel et désactivé par défaut.
  - [x] **Phase 1** : ~~Intégrer Silero VAD v6~~ — **Done.** Silero VAD v6 ONNX (~2.3 MB) embarqué via `include_bytes!`, inférence directe via `ort` (pas de crate VAD dédiée — conflits ndarray). Discard si pas de parole + trimming silences début/fin. Toggle `vad_enabled` dans Settings > Post-traitement (activé par défaut). Voir `cleanup/vad.rs`.
  - **Phase 2** : Denoising optionnel via nnnoiseless (pure Rust, 85 KB). Toggle dans préférences, désactivé par défaut. Si qualité insuffisante → DeepFilterNet3 (crate `deep_filter`, Rust natif via tract).
  - **Phase 3** : Presets device (gain, noise gate, normalisation par type de micro). Voir `docs/AUDIO-PIPELINE.md` Phase 3.
- [ ] **Restauration après crash** — Sauvegarder l'état de la queue de transcription sur disque (fichiers audio en attente). En cas de crash ou kill pendant une transcription, les fichiers WAV restent dans /tmp mais la queue en mémoire est perdue. Persister la queue permettrait de reprendre automatiquement au relancement. Concerne uniquement la transcription, pas le téléchargement.

## Intégrations Cloud & Modèles

- [ ] **Intégration Deepgram Nova-3** — API propriétaire mais simple (REST, audio brut en body, ~80 lignes Rust). Meilleure qualité sur audio bruité. Voir `docs/CLOUD-INTEGRATION.md`.
- [x] **Évaluer modèles de correction spécialisés** — **Done.** Évaluation complète dans `docs/BENCHMARK.md`. Métadonnées (params, RAM, langues, recommended) intégrées dans le sélecteur cleanup de Settings.
- [x] **Implémenter PCS Punctuation (47 langues)** — **Done.** Moteur `1-800-BAD-CODE/punct_cap_seg_47_language` (233 MB ONNX, 4 têtes : ponctuation, capitalisation, segmentation). Tokenizer SentencePiece parsé en Rust via `prost` (protobuf). Voir `cleanup/pcs.rs`, `engines/pcs.rs`.
- [x] **Intégrer modèles T5 de correction post-ASR** — **Done.** 4 modèles T5 (GEC T5 Small 60M multilingue, T5 Spell FR 220M, FlanEC Large 250M, Flan-T5 Grammar 783M). Inférence encoder-decoder via `candle-transformers` (Metal GPU, KV cache, décodage autorégressif). Nouvelle catégorie `Correction` (badge ambre). Voir `cleanup/t5.rs`, `engines/correction.rs`.

- [ ] **Post-processeur rule-based multilingue (style Harper)** — Rechercher et évaluer des correcteurs grammaticaux rule-based qui supportent plusieurs langues (pas seulement l'anglais). Harper est anglais-only mais l'approche est excellente : zéro ML, sub-milliseconde, corrections a/an, mots confondus, répétitions. Chercher des alternatives multilingues (LanguageTool Rust bindings ? hunspell ? autres crates ?). L'idée est une couche de polish ultra-rapide après la ponctuation PCS ou correction T5, qui attrape les erreurs grammaticales fréquentes de l'ASR sans overhead ML. Pourrait être une catégorie "RuleBased" séparée ou intégré comme post-processeur automatique.

## Refactoring

- [ ] **Regrouper les contextes d'inférence dans AppState** — On a maintenant 9 `Mutex<Option<...Context>>` individuels (`whisper_context`, `canary_context`, `parakeet_context`, `qwen_context`, `llm_context`, `bert_context`, `pcs_context`, `candle_punct_context`, `t5_context`), chacun avec sa propre logique de cache invalidation éparpillée. Refactorer en un `Mutex<InferenceCache>` unique avec méthodes centralisées (`invalidate_asr()`, `invalidate_punct()`, etc.). Simplifie l'ajout de futurs moteurs et rend l'invalidation croisée explicite.
- [x] **Splitter le store Pinia** — ~~`app.ts` centralise tout.~~ **Done.** Stores séparés : `app.ts` (runtime), `history.ts` (pagination backend + infinite scroll), `settings.ts`, `engines.ts`, `downloads.ts`.
- [x] **Restructuration modules Rust** — **Done.** Fichiers plats réorganisés en sous-modules : `asr/` (whisper, canary, parakeet, qwen, mel), `cleanup/` (bert, candle, pcs, t5, vad, llm_cloud, llm_local, post_processor, common), `engines/` (catalogue + registration), `platform/` (hotkey, permissions, paste, audio_devices, audio_ducking), `ui/` (tray, pill, menu_icons).

## Documentation

- [ ] **Guide de setup pour les utilisateurs** — Page `docs/SETUP-GUIDE.md` ou section README expliquant :
  - **Modèles natifs** — quels Whisper/LLM télécharger selon le hardware (RAM, Apple Silicon vs Intel)
  - **Cloud providers** — comment configurer Groq, OpenAI, Cerebras, Gemini (avec les presets)
  - **Serveurs locaux** — pour ceux qui veulent héberger un serveur séparé :
    - **LLM** : Ollama (`brew install ollama && ollama pull qwen3:4b`, URL `http://localhost:11434`) ou LM Studio (GUI, port 1234)
    - **ASR** : whisper.cpp server (`brew install whisper-cpp`, port 8080) ou MLX-Audio (`pip install mlx-audio`, port 8000)
  - Voir `docs/BENCHMARK.md` pour les comparatifs détaillés

## Technique / Infra

- [ ] **CI/CD GitHub Actions** — Pipeline automatique : bump de version → tag → build macOS (.app/.dmg) + Windows → release GitHub avec changelog auto-généré
- [ ] **CHANGELOG.md** — Fichier changelog versionné dans le repo
- [ ] **Script de test visuel + screenshots** — Flows de test automatisés (pill, settings, etc.) avec capture de screenshots
- [ ] **Windows support** — Implémenter les vrais bindings (hotkey via `SetWindowsHookEx`, permissions, paste, audio devices)

## Audits récurrents

À relancer après chaque grosse feature ou refactoring.

- [ ] **Audit spacings (padding/margin/hauteurs)** — Vérifier la cohérence des paddings de page, cards/items, inputs, boutons, space-y, et tailles de texte sur toutes les vues et composants.

- [ ] **Audit i18n** — Lancer `python3 scripts/audit-i18n.py` pour détecter clés orphelines, manquantes, dupliquées, et désync EN/FR. Compléter par une relecture manuelle pour les strings hardcodées.

- [ ] **Audit architecture & séparation des modules** — Passer en revue tout le codebase :
  - **Séparation des responsabilités** — chaque module a un rôle clair, pas de logique métier mélangée
  - **Duplication** — code dupliqué entre composants, patterns répétés
  - **Couplage** — dépendances entre modules, imports croisés
  - **Code mort** — fonctions/types/imports inutilisés après refactorings
  - **Frontend → backend** — logique métier côté JS qui devrait être côté Rust

- [ ] **Audit patterns & bonnes pratiques** — Vérifier la cohérence sur tout le projet :
  - **Error handling** — Result vs unwrap, propagation, messages clairs
  - **Sécurité** — clés API, sanitization, permissions
  - **Performance** — locks, allocations, I/O bloquant
  - **Concurrence** — Mutex/Atomics/channels, deadlocks, ordering
  - **Frontend** — listeners orphelins, memory leaks, cleanup onUnmounted
  - **Naming** — conventions (Rust snake_case, TS camelCase, events kebab-case)
  - **Tests** — couverture, zones critiques
