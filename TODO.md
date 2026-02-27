# WhisperDictate — TODO

## Bugs / Correctifs (Audit)

- [x] **CRITIQUE: Hotkey update tx dropped** — ✅ Fixé : sender stocké dans managed state, `set_setting` envoie `SetHotkey`/`SetCancelKey`
- [x] **CRITIQUE: Device UID mismatch** — ✅ Fixé : unifié sur CoreAudio UIDs dans `audio.rs`
- [ ] **TOCTOU races start/stop** — `is_recording` check-then-set sans garder le lock (`recording.rs:54-57, 77-80`). Garder le lock sur le check+set
- [ ] **CGEvent Unicode typing** — Remplacer le clipboard+Cmd+V par `CGEventKeyboardSetUnicodeString` pour taper le texte directement
- [ ] **`transcription-error` ne décrémente pas `queueCount`** — Le handler d'erreur oublie de décrémenter, `isBusy` reste true trop longtemps (`app.ts:359-361`)
- [ ] **Tray menu se ferme au premier clic après lancement** — Le tout premier clic sur l'icône tray après le démarrage de l'app ferme le menu au lieu de l'ouvrir

## Refacto (Audit)

- [ ] **17 Mutex individuels → grouper** — `state.rs` : regrouper en 2-3 groupes logiques (runtime, settings, history)
- [ ] **Duplicate audio device listing** — `audio.rs` (cpal) et `audio_devices.rs` (CoreAudio). Consolider sur CoreAudio
- [ ] **Regex recompilées en boucle** — `post_processor.rs:88-93`. Utiliser `LazyLock<Vec<Regex>>`
- [ ] **`unsafe impl Sync` sur RecordingState** — Fragile. Utiliser crossbeam channels ou garder le Mutex wrapper
- [ ] **`std::process::exit(0)` skip destructeurs** — `tray.rs`. Utiliser `app.exit(0)` ou finaliser avant
- [ ] **`AudioReply::Spectrum` dead code** — Supprimer (`recording.rs:35-36`)
- [ ] **Recursive async → while loop** — `process_next_in_queue` (`recording.rs:196`)
- [ ] **`reqwest::Client` recréé à chaque appel LLM** — Créer un client partagé
- [ ] **`VecDeque` pour la queue** — `state.rs:199`
- [ ] **`paste_text` bloque le runtime async** — Utiliser `spawn_blocking`
- [ ] **Guard double `init()`** — Protéger contre listeners dupliqués
- [ ] **Download state dual ownership** — Choisir promise OU event, pas les deux
- [ ] **Duplicate `spectrum-data` listener** — Event distinct pour mic test
- [ ] **Optimistic update sans rollback** — Restaurer l'état si invoke échoue
- [ ] **Clés i18n mortes** — `settings.postProcessing.llmConfigure` non utilisée
- [ ] **`selected_model_id`/`selected_language` redondant** — Dans get_settings ET get_app_state

## Fonctionnalités

- [ ] **Historique des transcriptions (infini)** — Stockage persistant (SQLite ou append-only), UI de consultation/recherche
- [ ] **Restauration après crash** — Sauvegarder l'état de la queue sur disque
- [ ] **Système de raccourcis personnalisés** — "Press to record" pour choisir n'importe quelle combinaison de touches
- [ ] **LLM post-processing : modèle local** — llama.cpp ou subprocess en plus des API distantes
- [ ] **Presets audio par type de device** — Gain, noise gate, normalisation selon micro intégré/AirPods/casque/USB

## UX / Polish

- [ ] **Indicateur visuel transcription dans le tray** — Feedback plus clair
- [ ] **Setup window : padding bouton Continuer** — Collé au bas de la fenêtre
- [ ] **Setup window : retirer texte redondant** — "pour WhisperDictate" superflu
- [ ] **Setup window : auto-restart** — Redémarrer auto au lieu du message "peut-être nécessaire"
- [ ] **Settings window : taille adaptative** — Hauteur prioritaire, colonnes si besoin

## Technique / Infra

- [ ] **Windows support** — Implémenter les vrais bindings (hotkey, permissions, paste, audio devices)
