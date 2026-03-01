# Intégration Cloud — Analyse et Priorités

Analyse des APIs cloud à intégrer dans WhisperDictate, au-delà de ce qui fonctionne déjà (OpenAI-compatible + Anthropic).

## Ce qui fonctionne déjà

### ASR Cloud
L'app envoie `POST /v1/audio/transcriptions` (multipart form, fichier WAV). Tout provider exposant ce endpoint fonctionne :
- **OpenAI** (whisper-1, gpt-4o-transcribe, gpt-4o-mini-transcribe)
- **Groq** (`https://api.groq.com/openai/v1`)
- **Fireworks AI** (`https://api.fireworks.ai/inference/v1`)
- **Together AI** (`https://api.together.xyz/v1`)

### LLM Cloud
L'app supporte deux formats :
- **OpenAI-compatible** `POST /v1/chat/completions` → Groq, Cerebras, Mistral, Google Gemini, Together, Fireworks, SambaNova, OpenRouter, DeepSeek
- **Anthropic Messages** `POST /v1/messages` → Claude Haiku/Sonnet

### Manque : presets provider

L'utilisateur doit actuellement configurer manuellement URL + clé + modèle. L'ajout de **presets préconfigurés** simplifierait énormément l'onboarding.

**Priorité : HAUTE** — C'est la fonctionnalité la plus impactante pour l'expérience cloud. Zéro changement backend, juste des presets frontend avec les bonnes URLs/modèles pré-remplis.

---

## APIs propriétaires à évaluer

### 1. Deepgram Nova-3

**Intérêt** : Meilleure qualité sur audio bruité/réel. Approche non-Whisper (architecture propriétaire). Anti-hallucination intégré.

**Format API** :
```
POST https://api.deepgram.com/v1/listen?model=nova-3&language=fr
Authorization: Token <api_key>
Content-Type: audio/wav
Body: [raw audio bytes]
```
**Réponse** : JSON avec `results.channels[0].alternatives[0].transcript`

**Difficulté** : Faible à moyenne
- Endpoint REST simple, pas de SDK requis
- Auth par header `Token` (pas Bearer)
- Body = audio brut (pas multipart comme OpenAI)
- Parsing réponse différent (nested JSON)
- ~50 lignes de code Rust (un nouveau `call_deepgram()` dans `llm_cleanup.rs` ou un nouveau module)

**Coût** : $0.004/min + $200 crédits gratuits

**Priorité : MOYENNE** — Bonne qualité, API simple, mais les options OpenAI-compatible couvrent déjà bien le besoin. Intéressant si un utilisateur a des problèmes de qualité sur audio bruité.

---

### 2. ElevenLabs Scribe v2

**Intérêt** : Très faible latence streaming (150ms). 90 langues.

**Format API** :
```
POST https://api.elevenlabs.io/v1/audio/transcriptions
Authorization: xi-api-key <api_key>
Content-Type: multipart/form-data
Body: file=@audio.wav, model=scribe_v2
```

**Difficulté** : Faible
- Format très proche d'OpenAI (multipart form)
- Juste le header d'auth qui change (`xi-api-key` au lieu de `Authorization: Bearer`)
- Réponse similaire

**Coût** : $0.006/min, plans avec minutes incluses

**Priorité : BASSE** — Pas de différenciation forte vs OpenAI-compatible existant. L'intérêt principal (streaming temps réel) ne s'applique pas à notre workflow batch.

---

### 3. Gladia (Whisper-Zero)

**Intérêt** : Whisper amélioré avec réduction d'hallucinations intégrée.

**Format API** :
```
POST https://api.gladia.io/v2/transcription
Headers: x-gladia-key: <api_key>
Content-Type: multipart/form-data
Body: audio=@file.wav
```
Réponse asynchrone : reçoit un `result_url` à poller.

**Difficulté** : Moyenne
- Pattern async (POST → poll URL pour résultat)
- Ajoute de la complexité au code (boucle de polling ou callback)

**Coût** : $0.010/min, 10h/mois gratuites

**Priorité : BASSE** — Le pattern async est pénalisant pour la dictée temps réel. L'anti-hallucination est intéressant mais on l'a déjà côté LLM.

---

### 4. AssemblyAI Universal-2

**Intérêt** : 99 langues, features avancées (diarization, entity detection).

**Format API** :
```
POST https://api.assemblyai.com/v2/transcript
Authorization: <api_key>
Content-Type: application/json
Body: { "audio_url": "https://..." }
```
Nécessite d'uploader l'audio d'abord, puis polling du résultat.

**Difficulté** : Élevée
- Workflow en 3 étapes : upload audio → create transcript → poll result
- Pas de mode synchrone simple
- Nécessite upload préalable de l'audio

**Coût** : $0.0025/min, $50 crédits gratuits

**Priorité : BASSE** — Trop complexe pour du dictation temps réel. Le workflow async 3 étapes n'est pas adapté.

---

### 5. Google Cloud Speech-to-Text (Chirp 3)

**Intérêt** : Qualité enterprise, 85+ langues.

**Difficulté** : Très élevée
- Nécessite un projet GCP, un service account, OAuth2 tokens
- SDK Google ou REST avec auth complexe
- Configuration lourde côté utilisateur

**Coût** : $0.016/min, 60 min/mois gratuit

**Priorité : TRÈS BASSE** — La complexité de setup (GCP project, IAM, service account JSON) est rédhibitoire pour un utilisateur final. Pas de valeur ajoutée vs Groq/OpenAI.

---

### 6. Azure Speech / Amazon Transcribe

**Difficulté** : Très élevée (SDK cloud, subscriptions, IAM)

**Priorité : TRÈS BASSE** — Mêmes problèmes que Google. Réservé aux entreprises avec infrastructure existante.

---

## Plan d'action recommandé

### Phase 1 — Presets provider (priorité haute, effort faible)

Ajouter des presets préconfigurés dans l'UI pour les providers OpenAI-compatible. Aucun changement backend, juste du frontend :

**ASR Cloud** :
- Groq → URL `https://api.groq.com/openai/v1`, modèle `whisper-large-v3-turbo`
- Fireworks → URL `https://api.fireworks.ai/inference/v1`, modèle `whisper-v3-turbo`
- OpenAI → URL `https://api.openai.com/v1`, modèle `gpt-4o-mini-transcribe`

**LLM Cloud** :
- Groq → URL `https://api.groq.com/openai/v1`, modèle `llama-3.1-8b-instant`
- Cerebras → URL `https://api.cerebras.ai/v1`, modèle `llama3.1-8b`
- OpenAI → URL `https://api.openai.com/v1`, modèle `gpt-4.1-nano`
- Google Gemini → URL `https://generativelanguage.googleapis.com/v1beta/openai`, modèle `gemini-2.5-flash-lite`
- Mistral → URL `https://api.mistral.ai/v1`, modèle `ministral-3b-latest`

### Phase 2 — Deepgram Nova-3 (priorité moyenne, effort moyen)

Si la qualité des options OpenAI-compatible ne suffit pas pour certains utilisateurs (audio bruité, accents forts), ajouter le support Deepgram :
- Nouveau module `deepgram.rs` ou variant dans `openai_api.rs`
- ~50-80 lignes de code
- Nouveau `ProviderKind::Deepgram` dans state.rs

### Phase 3 — Autres APIs propriétaires (priorité basse)

Seulement si demandé par les utilisateurs. ElevenLabs serait le plus simple à ajouter (format proche d'OpenAI).

---

## Résumé

| Action | Priorité | Effort | Impact |
|---|---|---|---|
| **Presets provider** | Haute | Faible (frontend seul) | Gros — simplifie l'onboarding cloud |
| **Deepgram Nova-3** | Moyenne | Moyen (~80 lignes Rust) | Moyen — qualité audio bruité |
| **ElevenLabs** | Basse | Faible (~30 lignes) | Faible — peu de différenciation |
| **Gladia** | Basse | Moyen (async polling) | Faible — anti-hallucination déjà couvert |
| **AssemblyAI** | Basse | Élevé (3 étapes) | Faible |
| **Google/Azure/AWS** | Très basse | Très élevé | Faible — setup rédhibitoire |

---

*Dernière mise à jour : mars 2026*
