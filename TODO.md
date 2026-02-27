# WhisperDictate — TODO

## Bugs / Correctifs

- [ ] **CGEvent Unicode typing** — Remplacer le clipboard+Cmd+V par `CGEventKeyboardSetUnicodeString` pour taper le texte directement (évite les conflits clipboard, pas de race condition)
- [ ] **Post-processing : toutes les options reviennent à l'état par défaut** — Les toggles (hallucination filter, LLM enabled) se réinitialisent si on navigue entre sections ou ferme/rouvre Settings (optimistic update appliqué, à vérifier)

## Fonctionnalités à implémenter

- [ ] **Système de raccourcis personnalisés** — Permettre de choisir n'importe quelle combinaison de touches (pas juste un dropdown de 4 options). Enregistrer un raccourci custom via un "press to record" UI
- [ ] **LLM post-processing : modèle local** — Support d'un modèle local (llama.cpp ou subprocess) en plus des API distantes (OpenAI/Anthropic)
- [ ] **Optimisation capture audio par type de device** — Adapter le traitement audio selon le type d'appareil (micro intégré Mac, AirPods, casque, micro fixe/USB). Possibilités : gain/amplification, noise gate, réduction de bruit, normalisation. Détecter le type de device et appliquer un profil adapté automatiquement

## UX / Polish

- [ ] **Tester la pill réduite (80x32)** — Vérifier que les animations (spectrum, dots, progress, error, badge) sont lisibles et proportionnées
- [ ] **Indicateur visuel pendant la transcription dans le tray** — Feedback plus clair que la dictée est en cours de traitement

## Technique / Infra

- [ ] **Windows support** — Les stubs platform existent, implémenter les vrais bindings Windows (hotkey, permissions, paste, audio devices)
- [ ] **Push des commits** — 12+ commits locaux en avance sur origin
