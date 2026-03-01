# WhisperDictate — TODO

## Bugs

- [ ] **LLM cleanup renvoie le system prompt** — Le nettoyage LLM renvoie parfois le prompt système dans la sortie au lieu du texte nettoyé uniquement. Bug dans le parsing de la réponse LLM (local ou cloud). À investiguer dans `llm_cleanup.rs` / `llm_local.rs`.
- [ ] **CGEvent Unicode typing** — Remplacer le clipboard+Cmd+V par `CGEventKeyboardSetUnicodeString` pour taper le texte directement
- [ ] **Tray menu se ferme au premier clic après lancement** — Bug upstream `tray-icon` (manque `acceptsFirstMouse:` sur TrayTarget NSView). Issue ouverte : tray-icon#251. Workaround actuel (menu attaché après build) est le meilleur disponible. Fix = PR upstream ou fork.

## Fonctionnalités

- [ ] **Raccourci pour historique rapide** — Touche configurable pour afficher un popup flottant avec les dernières transcriptions. Permet de re-coller rapidement un texte récent sans ouvrir la fenêtre d'historique complète. Style popup léger (comme Spotlight/Alfred), clic ou Enter pour coller l'entrée sélectionnée.
- [ ] **Détection de silence avant transcription** — Analyser l'audio avant de l'envoyer au modèle ASR. Si l'enregistrement ne contient que du silence (énergie RMS sous un seuil), le jeter directement sans transcrire. Évite les hallucinations sur audio vide (le modèle invente du texte quand il n'y a rien à transcrire).
- [ ] **Filtre hallucinations via LLM** — Envisager de remplacer ou compléter le filtre regex actuel par un passage LLM. Le LLM peut détecter contextuellement les hallucinations (répétitions, texte sans rapport, artefacts de fin) là où le regex ne catch que des patterns connus. Approche combinée possible : regex rapide d'abord, puis LLM pour les cas complexes.
- [ ] **Restauration après crash** — Sauvegarder l'état de la queue de transcription sur disque (fichiers audio en attente). En cas de crash ou kill pendant une transcription, les fichiers WAV restent dans /tmp mais la queue en mémoire est perdue. Persister la queue permettrait de reprendre automatiquement au relancement. Concerne uniquement la transcription, pas le téléchargement.
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
