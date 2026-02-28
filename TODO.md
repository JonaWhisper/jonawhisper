# WhisperDictate — TODO

## Bugs

- [ ] **CGEvent Unicode typing** — Remplacer le clipboard+Cmd+V par `CGEventKeyboardSetUnicodeString` pour taper le texte directement
- [ ] **Tray menu se ferme au premier clic après lancement** — Bug upstream `tray-icon` (manque `acceptsFirstMouse:` sur TrayTarget NSView). Issue ouverte : tray-icon#251. Workaround actuel (menu attaché après build) est le meilleur disponible. Fix = PR upstream ou fork.


## Fonctionnalités

- [x] **Moteur natif Rust whisper-rs** — whisper-rs compilé nativement, Metal GPU sur macOS. Vosk et Moonshine retirés (pas de binaire macOS / qualité inférieure). Subprocess Python supprimé. Téléchargement direct (reqwest + HTTP Range pour reprise).
- [x] **Système de providers unifié** — Providers = credentials purs (`id, name, kind, url, api_key`). Sélection de modèles centralisée dans Settings (ASR cloud + LLM). Model Manager simplifié (download/delete uniquement). Édition inline par card, ProviderForm réutilisable.
  - Reste à faire : branchement LLM local (llama.cpp) quand le catalogue de modèles locaux sera prêt.
- [ ] **Catalogue de modèles locaux** — Proposer un listing de modèles recommandés dans le Model Manager, couvrant ASR (speech-to-text) et post-processing (LLM cleanup). Permet aux utilisateurs de choisir sans connaître les noms de modèles.
  - **Modèles ASR locaux** — Whisper (tiny → large-v3) : afficher taille, langues, vitesse estimée.
  - **Modèles LLM post-processing locaux** — Listing de modèles légers pour le cleanup de texte en local :
    - Qwen3-1.7B-Instruct (~1 GB Q4, sweet spot qualité/taille)
    - SmolLM2-1.7B-Instruct (~1 GB Q4, Apache 2.0)
    - Gemma 3 1B-IT (~700 MB Q4, bon multilingual FR/EN)
    - Qwen3-4B-Instruct (~2.5 GB Q4, meilleure qualité)
    - Phi-4-mini 3.8B (~2.3 GB Q4, excellent reasoning)
  - **Modèles spécialisés ponctuation** — bert-restore-punctuation (~436 MB, sub-10ms), fullstop-punctuation-multilingual (~440 MB, FR/EN/DE/IT)
  - **Runtime local** — llama.cpp via [`llama-cpp-2`](https://github.com/utilityai/llama-cpp-rs) (Rust natif, Metal/CUDA, format GGUF) ou ONNX Runtime via [`ort`](https://github.com/pykeIO/ort) pour les modèles BERT
  - **Outils d'inférence locale** — Lister les logiciels recommandés pour faire tourner un LLM en local (tous compatibles API OpenAI) : [Ollama](https://ollama.com/), [LM Studio](https://lmstudio.ai/), [llama.cpp](https://github.com/ggerganov/llama.cpp), [vLLM](https://github.com/vllm-project/vllm)
- [ ] **Raccourci pour historique rapide** — Touche configurable pour afficher un popup flottant avec les dernières transcriptions. Permet de re-coller rapidement un texte récent sans ouvrir la fenêtre d'historique complète. Style popup léger (comme Spotlight/Alfred), clic ou Enter pour coller l'entrée sélectionnée.
- [ ] **Détection de silence avant transcription** — Analyser l'audio avant de l'envoyer au modèle ASR. Si l'enregistrement ne contient que du silence (énergie RMS sous un seuil), le jeter directement sans transcrire. Évite les hallucinations sur audio vide (le modèle invente du texte quand il n'y a rien à transcrire).
- [ ] **Filtre hallucinations via LLM** — Envisager de remplacer ou compléter le filtre regex actuel par un passage LLM. Le LLM peut détecter contextuellement les hallucinations (répétitions, texte sans rapport, artefacts de fin) là où le regex ne catch que des patterns connus. Approche combinée possible : regex rapide d'abord, puis LLM pour les cas complexes.
- [ ] **Restauration après crash** — Sauvegarder l'état de la queue de transcription sur disque (fichiers audio en attente). En cas de crash ou kill pendant une transcription, les fichiers WAV restent dans /tmp mais la queue en mémoire est perdue. Persister la queue permettrait de reprendre automatiquement au relancement. Concerne uniquement la transcription, pas le téléchargement.
- [x] **Reprise de téléchargement de modèles** — Fichier `.partial` stable par modèle + HTTP Range headers. Si l'app est tuée pendant un download, le fichier partiel reste sur disque et le téléchargement reprend automatiquement au prochain essai.
- [ ] **Presets audio par type de device** — Gain, noise gate, normalisation selon le micro utilisé. À réfléchir :
  - **Détection automatique** — Matcher le device par pattern dans le nom (ex: "AirPods" → preset Bluetooth, "MacBook" → preset intégré). Fournir quelques presets par défaut pour les cas courants.
  - **Presets personnalisés** — Permettre à l'utilisateur de créer/éditer ses propres presets et de les associer à un device spécifique. Important car chaque micro a ses particularités.

## Refactoring

- [ ] **Splitter le store Pinia** — `app.ts` centralise tout (état, downloads, settings, history, providers, permissions). Splitter en sous-stores thématiques : `useDownloadStore`, `useSettingsStore`, `useHistoryStore`, `useProviderStore`, etc. Chaque store gère son propre état + actions. Le store principal ne garde que l'état runtime (recording, transcription, queue).

## Technique / Infra

- [ ] **CI/CD GitHub Actions** — Pipeline automatique : bump de version → tag → build macOS (.app/.dmg) + Windows → release GitHub avec changelog auto-généré
- [ ] **CHANGELOG.md** — Fichier changelog versionné dans le repo
- [ ] **Script de test visuel + screenshots** — Flows de test automatisés (pill, settings, etc.) avec capture de screenshots
- [ ] **Windows support** — Implémenter les vrais bindings (hotkey via `SetWindowsHookEx`, permissions, paste, audio devices)

## Corrections spacings (audit du 28/02/2026)

Plan d'action par ordre de priorité :

1. [x] **Padding de page → `px-5` partout** — ModelManager `p-4` → `p-5`, History `px-4` → `px-5`.
2. [x] **Hauteur inputs → `h-9` partout** — Settings inputs LLM `h-8` → `h-9`. Vérifier que les selects shadcn ont la même hauteur.
3. [x] **Padding cards/items → 2 standards** — Normal (`px-4 py-3`) : ModelCell, permissions SetupWizard. Compact (`px-3 py-2`) : modèles SetupStep2, History entries. Supprimer les variantes `px-2.5 py-1.5` et `px-3.5 py-2.5`.
4. [x] **Réduire space-y → 3 valeurs** — Compact `space-y-1`, normal `space-y-2`, section `space-y-4`. Éliminer `space-y-0.5`, `space-y-1.5`, `space-y-3`, `space-y-3.5`.
5. [x] **Boutons download → même taille** — SetupStep2 `h-6` et ModelCell default `sm` → choisir un standard unique (`size="sm"`).
6. [x] **`text-[11px]` permissions → `text-xs`** — SetupWizard descriptions permissions. BenchmarkBadges `text-[10px]`/`text-[9px]` OK (spécifique).

## Corrections i18n (audit du 28/02/2026)

1. [x] **Localiser tooltips tray** — `tray.rs` : `"Recording…"` (l.371) → `t!("pill.recording")`, `"Transcribing…"` (l.376) → `t!("pill.transcribing")`, `"WhisperDictate"` (l.381, l.507) → `t!("app.name")`.
2. [x] **Localiser validation ApiServerForm** — `'Required'` en dur (l.32-34) → clé i18n `validation.required`.

## Audits récurrents

À relancer après chaque grosse feature ou refactoring.

- [ ] **Audit spacings (padding/margin/hauteurs)** — Vérifier la cohérence des paddings de page, cards/items, inputs, boutons, space-y, et tailles de texte sur toutes les vues et composants.

- [ ] **Audit i18n** — Vérifier que tout texte visible passe par `t()` (Vue) ou `t!()` (Rust). Chercher les strings hardcodées, clés manquantes, clés inutilisées, incohérences entre en.json et fr.json.

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
