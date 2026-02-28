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

## Spacings (audit fait)

- [ ] **Standardiser le padding de page** — ModelManager utilise `p-4`, Settings/SetupStep2 utilisent `p-5`, History utilise `px-4` variable. Passer tout à `px-5`.
- [ ] **Standardiser la hauteur des inputs** — ShortcutCapture = `h-9`, inputs LLM dans Settings = `h-8`, selects = default. Aligner tout sur `h-9` (36px).
- [ ] **Standardiser le padding des cards/items** — 4 valeurs différentes : ModelCell `px-4 py-3`, permissions Setup `px-3.5 py-2.5`, modèles SetupStep2 `px-2.5 py-1.5`, History `px-3 py-2`. Définir 2 standards : compact (`px-3 py-2`) et normal (`px-4 py-3`).
- [ ] **Réduire les variantes de space-y** — 7 valeurs utilisées (0.5 à 4). Garder 3 : compact `space-y-1`, normal `space-y-2`, section `space-y-4`.
- [ ] **Aligner les boutons download** — SetupStep2 utilise `h-6` (24px), ModelCell utilise default `sm` (~32px) pour la même action. Harmoniser.
- [ ] **Remplacer `text-[11px]` des descriptions permissions** — SetupWizard utilise `text-[11px]` custom, devrait être `text-xs` (12px) comme partout ailleurs. Les `text-[10px]`/`text-[9px]` dans BenchmarkBadges sont acceptables (besoin spécifique).

## i18n (audit fait)

- [ ] **Localiser les tooltips tray** — 4 strings hardcodées dans `tray.rs` : `"Recording…"` (l.371), `"Transcribing…"` (l.376), `"WhisperDictate"` (l.381, l.507). Remplacer par `t!("pill.recording")`, `t!("pill.transcribing")`, `t!("app.name")`.
- [ ] **Afficher les descriptions SetupStep2** — Les clés `setup.step2.hotkeyDesc`, `setup.step2.recordingModeDesc.push_to_talk`, `setup.step2.recordingModeDesc.toggle` existent dans en.json/fr.json mais ne sont jamais affichées dans le composant.
- [ ] **Localiser la validation ApiServerForm** — `'Required'` en dur (lignes 32-34). Remplacer par une clé i18n.

## Audits (à planifier)

- [ ] **Audit complet architecture & séparation des modules** — Passer en revue tout le codebase :
  - **Séparation des responsabilités** — chaque module a un rôle clair, pas de logique métier mélangée (ex: recording.rs ne devrait pas connaître le tray, commands.rs ne devrait pas contenir de logique)
  - **Duplication** — code dupliqué entre SetupStep2/Settings/ModelManager, tables de keycodes frontend/backend, patterns répétés
  - **Couplage** — dépendances entre modules, imports croisés, qui connaît qui
  - **Code mort** — fonctions/types/imports inutilisés après les refactorings successifs
  - **Frontend → backend** — logique faite côté JS qui devrait être côté Rust (filtrage, calculs, formatage). Le frontend = affichage, le Rust = logique métier.

- [ ] **Audit patterns & bonnes pratiques** — Vérifier la cohérence des patterns sur tout le projet :
  - **Error handling** — utilisation cohérente de Result vs unwrap, propagation d'erreurs, messages clairs
  - **Sécurité** — gestion des clés API, sanitization des inputs, permissions
  - **Performance** — locks trop larges, allocations inutiles, I/O bloquant sur le main thread
  - **Concurrence** — bon usage des Mutex/Atomics/channels, pas de deadlock possible, ordering des atomics
  - **Frontend** — listeners orphelins, memory leaks, réactivité, cleanup dans onUnmounted
  - **Naming** — conventions cohérentes (Rust snake_case, TS camelCase, events kebab-case)
  - **Tests** — couverture actuelle, zones critiques à tester en priorité
