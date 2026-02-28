# WhisperDictate — TODO

## Bugs

- [ ] **CGEvent Unicode typing** — Remplacer le clipboard+Cmd+V par `CGEventKeyboardSetUnicodeString` pour taper le texte directement
- [ ] **Tray menu se ferme au premier clic après lancement** — Bug upstream `tray-icon` (manque `acceptsFirstMouse:` sur TrayTarget NSView). Issue ouverte : tray-icon#251. Workaround actuel (menu attaché après build) est le meilleur disponible. Fix = PR upstream ou fork.


## Fonctionnalités

- [ ] **Moteurs natifs Rust (stratégie "zéro Python")** — Remplacer les subprocess Python/CLI par des bindings Rust natifs. Binaire unique sans dépendances externes, cross-platform (macOS + Windows).
  - **whisper.cpp → [whisper-rs](https://github.com/tazz4843/whisper-rs)** — Binding Rust natif, compilé par `build.rs`. CoreML/Metal sur macOS, CUDA/DirectML sur Windows. Moteur par défaut out-of-the-box. Sélection automatique du backend (GPU > Neural Engine > CPU). Option avancée dans Settings pour forcer (Auto / CPU / GPU).
  - **Vosk → [vosk-rs](https://github.com/Bear-03/vosk-rs)** — Binding Rust vers la lib C de Vosk. Léger, cross-platform.
  - **Moonshine → [ort](https://github.com/pykeIO/ort)** (ONNX Runtime) — Charge les modèles ONNX directement depuis Rust. Cross-platform, CPU/GPU/CoreML/DirectML.
  - **Faster Whisper → [ct2rs](https://github.com/jkawamoto/ctranslate2-rs)** — Binding CTranslate2. Moins mature, potentiellement redondant si whisper-rs + CoreML/Metal suffit.
  - **MLX Whisper** — Pas de binding Rust (framework Apple Python/Swift only). Remplacé par whisper-rs + CoreML. Garder en option Python legacy si besoin.
  - **Téléchargement des modèles** — Utiliser le crate officiel [hf-hub](https://github.com/huggingface/hf-hub) (Hugging Face) pour les modèles HF : cache, progression, reprise. Reqwest classique pour Vosk (alphacephei.com).
  - **Moteurs Python en fallback** — Garder le subprocess Python en option (venv dédié `~/.whisper-dictate/venv/`). Bouton "Install" dans le Model Manager qui gère le venv automatiquement. Plus le chemin par défaut.
- [ ] **Système de providers LLM unifié** — Fusionner `engines/ApiServerConfig` (transcription) et `LlmConfig` (cleanup) en un système unique. Chaque provider déclare ses capacités (audio→texte, texte→texte, ou les deux).
  - **Providers cloud pré-configurés** — OpenAI, Anthropic, Gemini : URL et modèles prédéfinis (dropdown), champs verrouillés. Custom uniquement pour serveurs locaux.
  - **Formulaire provider unifié** — Un seul composant Vue partagé entre Model Manager et Settings.
- [ ] **Catalogue de modèles locaux** — Proposer un listing de modèles recommandés dans le Model Manager, couvrant ASR (speech-to-text) et post-processing (LLM cleanup). Permet aux utilisateurs de choisir sans connaître les noms de modèles.
  - **Modèles ASR locaux** — Whisper (tiny → large-v3), Vosk (small/large), Moonshine (tiny/base) : afficher taille, langues, vitesse estimée.
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
- [ ] **Reprise de téléchargement de modèles** — Actuellement, quitter l'app pendant un download = progression perdue, il faut tout re-télécharger. Implémenter la reprise (HTTP Range headers pour les fichiers directs, cache natif de `hf-hub` pour HuggingFace). Fermer la fenêtre ne pose pas de problème (le download continue en arrière-plan), seul le quit de l'app est concerné.
- [ ] **Presets audio par type de device** — Gain, noise gate, normalisation selon le micro utilisé. À réfléchir :
  - **Détection automatique** — Matcher le device par pattern dans le nom (ex: "AirPods" → preset Bluetooth, "MacBook" → preset intégré). Fournir quelques presets par défaut pour les cas courants.
  - **Presets personnalisés** — Permettre à l'utilisateur de créer/éditer ses propres presets et de les associer à un device spécifique. Important car chaque micro a ses particularités.

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
