# WhisperDictate — TODO

## Bugs

- [ ] **CGEvent Unicode typing** — Remplacer le clipboard+Cmd+V par `CGEventKeyboardSetUnicodeString` pour taper le texte directement
- [ ] **Tray menu se ferme au premier clic après lancement** — Bug upstream `tray-icon` (manque `acceptsFirstMouse:` sur TrayTarget NSView). Issue ouverte : tray-icon#251. Workaround actuel (menu attaché après build) est le meilleur disponible. Fix = PR upstream ou fork.

## UX / Polish

- [ ] **Remplacer emojis/SVG inline par Lucide icons** — Unifier toutes les icones du projet avec `lucide-vue-next` (déjà installé). Fichiers concernés : Settings.vue (⚙ ✨ ⌨ 🎙 dans la sidebar), ModelCell.vue (SVG inline delete), SetupWizard.vue (SVG inline checkmark).
- [x] **Descriptions des moteurs dans le Model Manager** — Description par moteur affichée dans le panneau principal.
- [x] **Fix overflow fenêtres** — Remplacement `h-screen` par `h-full` + contraintes globales CSS (`html, body, #app { height: 100%; overflow: hidden }`).

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
- [x] **Historique des transcriptions (infini)** — Persistance SQLite (WAL), timeline groupée par jour, recherche, copier/supprimer
- [ ] **Restauration après crash** — Sauvegarder l'état de la queue sur disque
- [ ] **Système de raccourcis personnalisés** — "Press to record" pour choisir n'importe quelle combinaison de touches
- [ ] **Presets audio par type de device** — Gain, noise gate, normalisation selon micro intégré/AirPods/casque/USB

## Technique / Infra

- [ ] **CI/CD GitHub Actions** — Pipeline automatique : bump de version → tag → build macOS (.app/.dmg) + Windows → release GitHub avec changelog auto-généré
- [ ] **CHANGELOG.md** — Fichier changelog versionné dans le repo
- [ ] **Script de test visuel + screenshots** — Flows de test automatisés (pill, settings, etc.) avec capture de screenshots
- [ ] **Windows support** — Implémenter les vrais bindings (hotkey via `SetWindowsHookEx`, permissions, paste, audio devices)

## Audits (à planifier)

- [ ] **Audit frontend → backend** — Identifier la logique faite côté JavaScript qui serait mieux côté Rust via IPC (filtrage, calculs, formatage). Le frontend devrait idéalement ne faire que de l'affichage, le Rust gère la logique métier.

- [ ] **Audit global du codebase** — Re-vérifier l'ensemble après les refactorings récents (FFI, events, hotkey, crossbeam, mutex grouping). Couvrir :
  - **Sécurité** — injections, gestion des clés API, permissions, sanitization des inputs
  - **Performance** — allocations inutiles, locks trop larges, I/O bloquant sur le main thread
  - **Code quality** — patterns incohérents, dead code, error handling, unwrap justifiés
  - **Architecture** — couplage entre modules, séparation des responsabilités
  - **Frontend** — listeners orphelins, memory leaks, réactivité, accessibilité
  - **Tests** — couverture actuelle, identifier les zones critiques à tester en priorité
