# WhisperDictate — TODO

## Bugs / Correctifs

- [ ] **CGEvent Unicode typing** — Remplacer le clipboard+Cmd+V par `CGEventKeyboardSetUnicodeString` pour taper le texte directement (évite les conflits clipboard, pas de race condition)
- [ ] **Post-processing : toutes les options reviennent à l'état par défaut** — Les toggles (hallucination filter, LLM enabled) se réinitialisent si on navigue entre sections ou ferme/rouvre Settings (optimistic update appliqué, à vérifier)

## Fonctionnalités à implémenter

- [ ] **Système de raccourcis personnalisés** — Permettre de choisir n'importe quelle combinaison de touches (pas juste un dropdown de 4 options). Enregistrer un raccourci custom via un "press to record" UI
- [ ] **LLM post-processing : modèle local** — Support d'un modèle local (llama.cpp ou subprocess) en plus des API distantes (OpenAI/Anthropic)
- [ ] **Optimisation capture audio par type de device** — Système de presets audio par type d'appareil (micro intégré Mac, AirPods/écouteurs BT, casque filaire, micro USB/XLR). Chaque preset configure : gain/amplification, noise gate, réduction de bruit, normalisation. Presets par défaut fournis + l'utilisateur peut les personnaliser. Auto-détection du type de device pour appliquer le bon preset

## UX / Polish

- [x] **Pill réduite (80x32)** — Taille validée, animations proportionnées OK
- [ ] **Indicateur visuel pendant la transcription dans le tray** — Feedback plus clair que la dictée est en cours de traitement

## Technique / Infra

- [ ] **Windows support** — Les stubs platform existent, implémenter les vrais bindings Windows (hotkey, permissions, paste, audio devices)
- [x] **Push des commits** — 17 commits pushés
