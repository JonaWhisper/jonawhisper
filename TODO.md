# WhisperDictate — TODO

## Bugs / Correctifs (Audit)

- [x] **CRITIQUE: Hotkey update tx dropped** — ✅ Fixé : sender stocké dans managed state, `set_setting` envoie `SetHotkey`/`SetCancelKey`
- [x] **CRITIQUE: Device UID mismatch** — ✅ Fixé : unifié sur CoreAudio UIDs dans `audio.rs`
- [x] **TOCTOU races start/stop** — ✅ Fixé : lock gardé sur check+set dans start/stop/process_next
- [ ] **CGEvent Unicode typing** — Remplacer le clipboard+Cmd+V par `CGEventKeyboardSetUnicodeString` pour taper le texte directement
- [x] **`transcription-error` ne décrémente pas `queueCount`** — ✅ Fixé : queueCount décrémenté dans le handler d'erreur
- [ ] **Tray menu se ferme au premier clic après lancement** — Le tout premier clic sur l'icône tray après le démarrage de l'app ferme le menu au lieu de l'ouvrir
- [ ] **Test micro vs enregistrement** — Si le test micro est actif quand une transcription démarre, annuler le test micro automatiquement

## Refacto (Audit)

- [x] **17 Mutex individuels → grouper** — ✅ Fixé : 4 groupes (runtime, download, settings, history), lock+copy+drop partout
- [x] **Duplicate audio device listing** — ✅ Fixé : un seul AudioDevice (CoreAudio), filtré par cpal dans audio::list_usable_devices()
- [x] **Regex recompilées en boucle** — ✅ Fixé : LazyLock pour hallucinations et dictation commands
- [x] **`unsafe impl Sync` sur RecordingState** — ✅ Fixé : crossbeam-channel (Send+Sync), unsafe impls supprimés
- [x] **`std::process::exit(0)` skip destructeurs** — ✅ Fixé : remplacé par `app.exit(0)`
- [x] **`AudioReply::Spectrum` dead code** — ✅ Supprimé
- [x] **Recursive async → while loop** — ✅ Fixé : boucle while dans process_next_in_queue
- [x] **`reqwest::Client` recréé à chaque appel LLM** — ✅ Fixé : LazyLock<reqwest::Client>
- [x] **`VecDeque` pour la queue** — ✅ Fixé : VecDeque avec push_back/pop_front
- [x] **`paste_text` bloque le runtime async** — ✅ Fixé : exécuté via spawn_blocking
- [x] **Guard double `init()`** — ✅ Fixé : flag `initialized` empêche les listeners dupliqués
- [x] **Download state dual ownership** — ✅ Fixé : retiré download-complete listener, promise seule gère la fin
- [x] **Duplicate `spectrum-data` listener** — ✅ Fixé : event `mic-test-spectrum` séparé
- [x] **Optimistic update sans rollback** — ✅ Fixé : rollback automatique sur échec invoke
- [x] **Clés i18n mortes** — ✅ Supprimé `settings.postProcessing.llmConfigure`
- [x] **`selected_model_id`/`selected_language` redondant** — ✅ Fixé : retiré de get_app_state, unique dans get_settings

## Fonctionnalités

- [ ] **Historique des transcriptions (infini)** — Stockage persistant (SQLite ou append-only), UI de consultation/recherche
- [ ] **Restauration après crash** — Sauvegarder l'état de la queue sur disque
- [ ] **Système de raccourcis personnalisés** — "Press to record" pour choisir n'importe quelle combinaison de touches
- [ ] **LLM post-processing : modèle local** — llama.cpp ou subprocess en plus des API distantes
- [ ] **Presets audio par type de device** — Gain, noise gate, normalisation selon micro intégré/AirPods/casque/USB

## UX / Polish

- [ ] **Indicateur visuel transcription dans le tray** — Feedback plus clair
- [x] **Setup window : padding bouton Continuer** — ✅ Fixé : ajouté pb-2
- [x] **Setup window : retirer texte redondant** — ✅ Fixé : "pour WhisperDictate" retiré du subtitle
- [x] **Setup window : note restart supprimée** — ✅ Fixé : note inutile retirée (startMonitoring gère tout)
- [ ] **Settings window : taille adaptative** — Hauteur prioritaire, colonnes si besoin

## Technique / Infra

- [ ] **Script de test visuel + screenshots** — Pouvoir lancer des flows de test (pill, settings, etc.) et capturer des screenshots automatiquement pour vérifier le rendu sans intervention manuelle
- [ ] **Windows support** — Implémenter les vrais bindings (hotkey, permissions, paste, audio devices)
