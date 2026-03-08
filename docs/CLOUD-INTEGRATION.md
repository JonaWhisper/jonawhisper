# Cloud Integration

APIs cloud pour ASR et LLM dans JonaWhisper.

## Architecture actuelle

### Providers

Un `Provider` = un fournisseur cloud configuré par l'utilisateur (clé API + URL). Le `ProviderKind` enum identifie le fournisseur, et les presets dans `src/config/providers.ts` déclarent les modèles disponibles par catégorie (ASR / LLM).

**Détection ASR vs LLM** : pas de flag explicite sur le provider. Le système utilise :
- Les presets (`asrModels` / `llmModels` par `ProviderKind`)
- Une heuristique sur le nom du modèle (`whisper`, `transcrib` = ASR)
- Les providers Custom sont considérés comme supportant les deux

### Protocoles supportés

| Protocole | Endpoint | Providers |
|-----------|----------|-----------|
| **OpenAI-compatible ASR** | `POST /v1/audio/transcriptions` (multipart) | OpenAI, Groq, Fireworks, Together |
| **OpenAI-compatible LLM** | `POST /v1/chat/completions` | OpenAI, Groq, Cerebras, Gemini, Mistral, Fireworks, Together, DeepSeek |
| **Anthropic Messages** | `POST /v1/messages` | Anthropic |

### Sécurité

- Clés API dans le **Keychain macOS** (`keyring` v3), jamais sur disque
- IPC `get_providers` retourne des clés masquées (`••••abcd`)
- HTTPS obligatoire sauf `allow_insecure` pour Custom (serveurs locaux)

---

## Presets

9 providers préconfigurés. L'utilisateur entre juste sa clé API.

| Provider | ASR | LLM | URL |
|----------|-----|-----|-----|
| **OpenAI** | whisper-1, gpt-4o-transcribe, gpt-4o-mini-transcribe | gpt-4o-mini, gpt-4o | `api.openai.com` |
| **Groq** | whisper-large-v3-turbo, whisper-large-v3 | llama-3.1-8b-instant | `api.groq.com` |
| **Fireworks** | whisper-v3-turbo, whisper-v3 | -- | `api.fireworks.ai` |
| **Together** | openai/whisper-large-v3 | meta-llama/Llama-3.2-3B | `api.together.xyz` |
| **Anthropic** | -- | claude-haiku-4-5, claude-sonnet-4-5, claude-opus-4-6 | `api.anthropic.com` |
| **Cerebras** | -- | llama3.1-8b | `api.cerebras.ai` |
| **Gemini** | -- | gemini-2.5-flash-lite | `generativelanguage.googleapis.com` |
| **Mistral** | -- | ministral-3b-latest | `api.mistral.ai` |
| **DeepSeek** | -- | deepseek-v3.2 | `api.deepseek.com` |

---

## APIs propriétaires à intégrer

APIs ASR qui ne suivent pas le format OpenAI et nécessitent un adaptateur dédié.

### Deepgram Nova-3

| | |
|---|---|
| **Intérêt** | Meilleure qualité sur audio bruité, anti-hallucination intégré |
| **Endpoint** | `POST https://api.deepgram.com/v1/listen?model=nova-3&language=fr` |
| **Auth** | `Authorization: Token <key>` (pas Bearer) |
| **Body** | Raw audio bytes (`Content-Type: audio/wav`) |
| **Réponse** | `results.channels[0].alternatives[0].transcript` |
| **Coût** | $0.004/min + $200 crédits gratuits |
| **Effort** | ~50-80 lignes Rust, nouveau `ProviderKind::Deepgram` |
| **Priorité** | **Moyenne** |

### ElevenLabs Scribe v2

| | |
|---|---|
| **Intérêt** | 90 langues, très faible latence |
| **Endpoint** | `POST https://api.elevenlabs.io/v1/audio/transcriptions` |
| **Auth** | `xi-api-key: <key>` |
| **Body** | Multipart form (`file=@audio.wav, model=scribe_v2`) |
| **Réponse** | Format proche OpenAI |
| **Coût** | $0.006/min |
| **Effort** | ~30 lignes, format très proche OpenAI (header d'auth différent) |
| **Priorité** | **Basse** -- peu de différenciation |

### Gladia (Whisper-Zero)

| | |
|---|---|
| **Intérêt** | Whisper amélioré, réduction hallucinations |
| **Endpoint** | `POST https://api.gladia.io/v2/transcription` |
| **Auth** | `x-gladia-key: <key>` |
| **Body** | Multipart form |
| **Réponse** | Asynchrone : `result_url` à poller |
| **Coût** | $0.010/min, 10h/mois gratuites |
| **Effort** | Moyen (polling async) |
| **Priorité** | **Basse** -- pattern async pénalisant pour dictation |

### AssemblyAI Universal-2

| | |
|---|---|
| **Intérêt** | 99 langues, diarization |
| **Endpoint** | `POST https://api.assemblyai.com/v2/transcript` |
| **Auth** | `Authorization: <key>` |
| **Body** | JSON `{ "audio_url": "..." }` -- upload préalable requis |
| **Réponse** | Asynchrone 3 étapes (upload → create → poll) |
| **Coût** | $0.0025/min |
| **Effort** | Élevé (workflow 3 étapes) |
| **Priorité** | **Basse** |

### Google Cloud Speech / Azure / AWS

| | |
|---|---|
| **Effort** | Très élevé (SDK cloud, service accounts, IAM) |
| **Priorité** | **Très basse** -- setup rédhibitoire pour utilisateur final |

---

## Résumé des priorités

| API | Type | Priorité | Effort | Notes |
|-----|------|----------|--------|-------|
| Presets (9 providers) | ASR + LLM | -- | -- | **Fait** |
| Deepgram Nova-3 | ASR | Moyenne | ~80 lignes | Qualité audio bruité |
| ElevenLabs Scribe | ASR | Basse | ~30 lignes | Peu de différenciation |
| Gladia | ASR | Basse | Moyen | Async, pénalisant |
| AssemblyAI | ASR | Basse | Élevé | 3 étapes async |
| Google/Azure/AWS | ASR | Très basse | Très élevé | Setup trop complexe |

*Dernière mise à jour : mars 2026*
