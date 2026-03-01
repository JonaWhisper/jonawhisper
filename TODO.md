# WhisperDictate — TODO

## Bugs

- [ ] **CGEvent Unicode typing** — Remplacer le clipboard+Cmd+V par `CGEventKeyboardSetUnicodeString` pour taper le texte directement
- [ ] **Tray menu se ferme au premier clic après lancement** — Bug upstream `tray-icon` (manque `acceptsFirstMouse:` sur TrayTarget NSView). Issue ouverte : tray-icon#251. Workaround actuel (menu attaché après build) est le meilleur disponible. Fix = PR upstream ou fork.

## Fonctionnalités

- [ ] **Raccourci pour historique rapide** — Touche configurable pour afficher un popup flottant avec les dernières transcriptions. Permet de re-coller rapidement un texte récent sans ouvrir la fenêtre d'historique complète. Style popup léger (comme Spotlight/Alfred), clic ou Enter pour coller l'entrée sélectionnée.
- [ ] **Pipeline prétraitement audio** — VAD + denoising optionnel avant transcription. Voir `docs/AUDIO-PIPELINE.md` pour l'architecture complète. **Important** : le denoising dégrade Whisper (paper "When De-noising Hurts", arXiv:2512.17562) → VAD prioritaire, denoising optionnel et désactivé par défaut.
  - **Phase 1** : Intégrer Silero VAD v6 (crate `voice_activity_detector`, 2 MB ONNX, `ort` déjà dispo) pour détecter le silence → discard si pas de parole + trimming silences début/fin.
  - **Phase 2** : Denoising optionnel via nnnoiseless (pure Rust, 85 KB). Toggle dans préférences, désactivé par défaut. Si qualité insuffisante → DeepFilterNet3 (crate `deep_filter`, Rust natif via tract).
  - **Phase 3** : Presets device (gain, noise gate, normalisation par type de micro). Voir `docs/AUDIO-PIPELINE.md` Phase 3.
- [ ] **Restauration après crash** — Sauvegarder l'état de la queue de transcription sur disque (fichiers audio en attente). En cas de crash ou kill pendant une transcription, les fichiers WAV restent dans /tmp mais la queue en mémoire est perdue. Persister la queue permettrait de reprendre automatiquement au relancement. Concerne uniquement la transcription, pas le téléchargement.

## Intégrations Cloud & Modèles

- [x] **Presets provider préconfigurés** — 9 providers préconfigurés (OpenAI, Anthropic, Groq, Cerebras, Gemini, Mistral, Fireworks, Together, DeepSeek). URLs résolues dynamiquement via `ProviderKind::base_url()` (pas de migration si les URLs changent). Bouton Test pour valider la clé API + récupérer les modèles disponibles. Cache des modèles sur le Provider (persiste entre restarts). Boutons Refresh dans Settings pour actualiser les listes.
- [x] **Enrichir le catalogue LLM natif** — Ajouté 6 modèles GGUF : Qwen3 0.6B, Llama 3.2 1B/3B, SmolLM3 3B, Ministral 3 3B, Gemma 3 4B (11 modèles total)
- [x] **Ajouter whisper-large-v3-french au catalogue ASR** — `bofenghuang/whisper-large-v3-french-distil-dec2` (GGML q5_0, 538 MB, spécialisé FR)
- [ ] **Intégration Deepgram Nova-3** — API propriétaire mais simple (REST, audio brut en body, ~80 lignes Rust). Meilleure qualité sur audio bruité. Voir `docs/CLOUD-INTEGRATION.md`.
- [ ] **Évaluer modèles de correction spécialisés** — Alternative au LLM pour le text cleanup : pipeline léger (regex filler words → ponctuation ONNX → grammar). Modèles candidats : `1-800-BAD-CODE/punct_cap_seg_47_language` (47 langues, F1=97%), `fdemelo/t5-base-spell-correction-fr` (correction FR), `FlanEC` (post-ASR error correction). Voir `docs/BENCHMARK.md` section "Modèles de correction spécialisés".

## Refactoring

- [ ] **Splitter le store Pinia** — `app.ts` centralise tout (état, downloads, settings, history, providers, permissions). Splitter en sous-stores thématiques : `useDownloadStore`, `useSettingsStore`, `useHistoryStore`, `useProviderStore`, etc. Chaque store gère son propre état + actions. Le store principal ne garde que l'état runtime (recording, transcription, queue).

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
