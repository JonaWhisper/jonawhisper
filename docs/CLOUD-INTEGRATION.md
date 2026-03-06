# Cloud Integration

APIs cloud pour ASR et LLM dans JonaWhisper.

## Architecture actuelle

### Providers

Un `Provider` = un fournisseur cloud configur\u00e9 par l'utilisateur (cl\u00e9 API + URL). Le `ProviderKind` enum identifie le fournisseur, et les presets dans `src/config/providers.ts` d\u00e9clarent les mod\u00e8les disponibles par cat\u00e9gorie (ASR / LLM).

**D\u00e9tection ASR vs LLM** : pas de flag explicite sur le provider. Le syst\u00e8me utilise :
- Les presets (`asrModels` / `llmModels` par `ProviderKind`)
- Une heuristique sur le nom du mod\u00e8le (`whisper`, `transcrib` = ASR)
- Les providers Custom sont consid\u00e9r\u00e9s comme supportant les deux

### Protocoles support\u00e9s

| Protocole | Endpoint | Providers |
|-----------|----------|-----------|
| **OpenAI-compatible ASR** | `POST /v1/audio/transcriptions` (multipart) | OpenAI, Groq, Fireworks, Together |
| **OpenAI-compatible LLM** | `POST /v1/chat/completions` | OpenAI, Groq, Cerebras, Gemini, Mistral, Fireworks, Together, DeepSeek |
| **Anthropic Messages** | `POST /v1/messages` | Anthropic |

### S\u00e9curit\u00e9

- Cl\u00e9s API dans le **Keychain macOS** (`keyring` v3), jamais sur disque
- IPC `get_providers` retourne des cl\u00e9s masqu\u00e9es (`\u2022\u2022\u2022\u2022abcd`)
- HTTPS obligatoire sauf `allow_insecure` pour Custom (serveurs locaux)

---

## Presets

9 providers pr\u00e9configur\u00e9s. L'utilisateur entre juste sa cl\u00e9 API.

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

## APIs propri\u00e9taires \u00e0 int\u00e9grer

APIs ASR qui ne suivent pas le format OpenAI et n\u00e9cessitent un adaptateur d\u00e9di\u00e9.

### Deepgram Nova-3

| | |
|---|---|
| **Int\u00e9r\u00eat** | Meilleure qualit\u00e9 sur audio bruit\u00e9, anti-hallucination int\u00e9gr\u00e9 |
| **Endpoint** | `POST https://api.deepgram.com/v1/listen?model=nova-3&language=fr` |
| **Auth** | `Authorization: Token <key>` (pas Bearer) |
| **Body** | Raw audio bytes (`Content-Type: audio/wav`) |
| **R\u00e9ponse** | `results.channels[0].alternatives[0].transcript` |
| **Co\u00fbt** | $0.004/min + $200 cr\u00e9dits gratuits |
| **Effort** | ~50-80 lignes Rust, nouveau `ProviderKind::Deepgram` |
| **Priorit\u00e9** | **Moyenne** |

### ElevenLabs Scribe v2

| | |
|---|---|
| **Int\u00e9r\u00eat** | 90 langues, tr\u00e8s faible latence |
| **Endpoint** | `POST https://api.elevenlabs.io/v1/audio/transcriptions` |
| **Auth** | `xi-api-key: <key>` |
| **Body** | Multipart form (`file=@audio.wav, model=scribe_v2`) |
| **R\u00e9ponse** | Format proche OpenAI |
| **Co\u00fbt** | $0.006/min |
| **Effort** | ~30 lignes, format tr\u00e8s proche OpenAI (header d'auth diff\u00e9rent) |
| **Priorit\u00e9** | **Basse** -- peu de diff\u00e9renciation |

### Gladia (Whisper-Zero)

| | |
|---|---|
| **Int\u00e9r\u00eat** | Whisper am\u00e9lior\u00e9, r\u00e9duction hallucinations |
| **Endpoint** | `POST https://api.gladia.io/v2/transcription` |
| **Auth** | `x-gladia-key: <key>` |
| **Body** | Multipart form |
| **R\u00e9ponse** | Asynchrone : `result_url` \u00e0 poller |
| **Co\u00fbt** | $0.010/min, 10h/mois gratuites |
| **Effort** | Moyen (polling async) |
| **Priorit\u00e9** | **Basse** -- pattern async p\u00e9nalisant pour dictation |

### AssemblyAI Universal-2

| | |
|---|---|
| **Int\u00e9r\u00eat** | 99 langues, diarization |
| **Endpoint** | `POST https://api.assemblyai.com/v2/transcript` |
| **Auth** | `Authorization: <key>` |
| **Body** | JSON `{ "audio_url": "..." }` -- upload pr\u00e9alable requis |
| **R\u00e9ponse** | Asynchrone 3 \u00e9tapes (upload \u2192 create \u2192 poll) |
| **Co\u00fbt** | $0.0025/min |
| **Effort** | \u00c9lev\u00e9 (workflow 3 \u00e9tapes) |
| **Priorit\u00e9** | **Basse** |

### Google Cloud Speech / Azure / AWS

| | |
|---|---|
| **Effort** | Tr\u00e8s \u00e9lev\u00e9 (SDK cloud, service accounts, IAM) |
| **Priorit\u00e9** | **Tr\u00e8s basse** -- setup r\u00e9dhibitoire pour utilisateur final |

---

## R\u00e9sum\u00e9 des priorit\u00e9s

| API | Type | Priorit\u00e9 | Effort | Notes |
|-----|------|----------|--------|-------|
| Presets (9 providers) | ASR + LLM | -- | -- | **Fait** |
| Deepgram Nova-3 | ASR | Moyenne | ~80 lignes | Qualit\u00e9 audio bruit\u00e9 |
| ElevenLabs Scribe | ASR | Basse | ~30 lignes | Peu de diff\u00e9renciation |
| Gladia | ASR | Basse | Moyen | Async, p\u00e9nalisant |
| AssemblyAI | ASR | Basse | \u00c9lev\u00e9 | 3 \u00e9tapes async |
| Google/Azure/AWS | ASR | Tr\u00e8s basse | Tr\u00e8s \u00e9lev\u00e9 | Setup trop complexe |

*Derni\u00e8re mise \u00e0 jour : mars 2026*
