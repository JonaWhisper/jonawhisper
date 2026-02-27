# WhisperDictate — TODO

## Bugs / Correctifs (Audit)

- [ ] **CRITIQUE: Hotkey update tx dropped** — `_hotkey_update_tx` est ignoré dans `lib.rs:88`, changer le raccourci/cancel dans Settings n'a aucun effet jusqu'au redémarrage. Stocker le sender dans le state Tauri et envoyer `SetHotkey`/`SetCancelKey` depuis `set_setting`
- [ ] **CRITIQUE: Device UID mismatch** — Le tray utilise CoreAudio UIDs réels, le recorder (`audio.rs`) match par nom cpal. La sélection de device retombe toujours sur le défaut. Unifier sur CoreAudio UIDs partout
- [ ] **TOCTOU races start/stop** — `is_recording` check-then-set sans garder le lock (`recording.rs:54-57, 77-80`). Garder le lock sur le check+set
- [ ] **CGEvent Unicode typing** — Remplacer le clipboard+Cmd+V par `CGEventKeyboardSetUnicodeString` pour taper le texte directement
- [ ] **`transcription-error` ne décrémente pas `queueCount`** — Le handler d'erreur oublie de décrémenter, `isBusy` reste true trop longtemps (`app.ts:359-361`)
- [ ] **Tray menu se ferme au premier clic après lancement** — Le tout premier clic sur l'icône tray après le démarrage de l'app ferme le menu au lieu de l'ouvrir. Fonctionne normalement ensuite
- [x] **Post-processing : persistence des toggles** — Bug trouvé : Switch utilisait `:checked` au lieu de `:model-value` (API reka-ui v2)

## Refacto (Audit)

- [ ] **17 Mutex individuels → grouper** — `state.rs` : regrouper les champs en 2-3 groupes logiques (runtime state, user settings, history) au lieu d'un Mutex par champ
- [ ] **Duplicate audio device listing** — `audio.rs` (cpal) et `audio_devices.rs` (CoreAudio) font la même chose. Consolider sur la version CoreAudio (supérieure : vrais UIDs, transport type)
- [ ] **Regex recompilées en boucle** — `post_processor.rs:88-93` : hallucination + dictation regexes recompilées à chaque appel. Utiliser `LazyLock<Vec<Regex>>`
- [ ] **`unsafe impl Sync` sur RecordingState** — Fragile (`recording.rs:47-49`). Retirer l'impl et utiliser crossbeam channels ou garder le Mutex wrapper
- [ ] **`std::process::exit(0)` skip destructeurs** — `tray.rs:463` : WAV en cours pourrait être corrompu. Utiliser `app.exit(0)` ou finaliser avant
- [ ] **`save_preferences()` : log les erreurs** — Le write disque était silencieux, maintenant logué (fait). Considérer un feedback UI si l'écriture échoue
- [ ] **`AudioReply::Spectrum` jamais utilisé** — Dead code dans `recording.rs:35-36`, supprimer
- [ ] **Recursive async → while loop** — `process_next_in_queue` utilise `Box::pin` recursif, remplacer par une boucle (`recording.rs:196`)
- [ ] **`reqwest::Client` recréé à chaque appel LLM** — `llm_cleanup.rs:117,157`. Créer un client partagé
- [ ] **`VecDeque` pour la queue** — `state.rs:199` : `Vec::remove(0)` est O(n), utiliser `VecDeque::pop_front()`
- [ ] **`paste_text` bloque le runtime async** — `paste.rs:12,17` : `thread::sleep` dans un contexte async. Utiliser `spawn_blocking`
- [ ] **Guard double `init()`** — `app.ts` : pas de protection contre double appel de `setupListeners()` (listeners dupliqués en HMR)
- [ ] **Download state dual ownership** — `app.ts:214-230` : état géré par promise `finally` ET event listener. Choisir un seul mécanisme
- [ ] **Duplicate `spectrum-data` listener** — Mic test et pill reçoivent le même event, se polluent. Utiliser un event distinct pour le mic test
- [ ] **Optimistic update sans rollback** — `app.ts:261-277` : si l'invoke échoue, l'état local n'est pas restauré
- [ ] **Clés i18n mortes** — `settings.postProcessing.llmConfigure` définie mais non utilisée
- [ ] **`selected_model_id`/`selected_language` dans get_settings ET get_app_state** — Redondant, nettoyer

## Fonctionnalités à implémenter

- [ ] **Historique des transcriptions (infini)** — Stocker l'historique de façon persistante (SQLite ou fichier append-only), pas de limite de 20 entrées. UI pour consulter/rechercher l'historique
- [ ] **Restauration après crash** — Si l'app se bloque pendant une transcription, restaurer l'output en cours au redémarrage (sauvegarder l'état de la queue sur disque)
- [ ] **Système de raccourcis personnalisés** — Permettre de choisir n'importe quelle combinaison de touches (pas juste un dropdown de 4 options). Enregistrer un raccourci custom via un "press to record" UI
- [ ] **LLM post-processing : modèle local** — Support d'un modèle local (llama.cpp ou subprocess) en plus des API distantes (OpenAI/Anthropic)
- [ ] **Optimisation capture audio par type de device** — Système de presets audio par type d'appareil (micro intégré Mac, AirPods/écouteurs BT, casque filaire, micro USB/XLR). Chaque preset configure : gain/amplification, noise gate, réduction de bruit, normalisation. Presets par défaut fournis + l'utilisateur peut les personnaliser. Auto-détection du type de device pour appliquer le bon preset

## UX / Polish

- [x] **Pill réduite (80x32)** — Taille validée, animations proportionnées OK
- [ ] **Indicateur visuel pendant la transcription dans le tray** — Feedback plus clair que la dictée est en cours de traitement
- [ ] **Setup window : padding bouton Continuer** — Le bouton est collé au bas de la fenêtre, ajouter du padding
- [ ] **Setup window : retirer texte redondant** — Le sous-titre "pour WhisperDictate" est superflu (déjà dans le titre)
- [ ] **Setup window : auto-restart** — Redémarrer automatiquement l'app quand les permissions nécessitent un restart, au lieu du message "redémarrage peut-être nécessaire"
- [ ] **Settings window : taille adaptative** — Ajuster la fenêtre au contenu (hauteur prioritaire, passer en colonnes si la hauteur d'écran est insuffisante)

## Technique / Infra

- [ ] **Windows support** — Les stubs platform existent, implémenter les vrais bindings Windows (hotkey, permissions, paste, audio devices)
