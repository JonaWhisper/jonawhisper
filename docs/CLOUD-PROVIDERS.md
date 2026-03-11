# Cloud Providers — Référence complète

Référence unique pour toutes les APIs cloud pertinentes pour JonaWhisper (ASR et LLM).

---

## Architecture

### Protocoles supportés

| Protocole | Endpoint | Backend |
|-----------|----------|---------|
| **OpenAI-compatible ASR** | `POST /v1/audio/transcriptions` (multipart) | `jona-provider-openai` |
| **OpenAI-compatible LLM** | `POST /v1/chat/completions` | `jona-provider-openai` |
| **Anthropic Messages** | `POST /v1/messages` | `jona-provider-anthropic` |

### Détection ASR vs LLM

Chaque preset déclare explicitement ses capacités via `ProviderPreset.supports_asr` et `supports_llm` (système inventory dans le crate `jona-provider-openai`). Pour les providers Custom, les deux sont considérés comme supportés.

### Sécurité

- Clés API dans le **Keychain macOS** (`keyring` v3), jamais sur disque
- IPC `get_providers` retourne des clés masquées (`••••abcd`)
- HTTPS obligatoire sauf `allow_insecure` pour Custom (serveurs locaux)

---

## Cloud ASR — OpenAI-compatible (intégré)

L'app supporte `POST /v1/audio/transcriptions` avec multipart form. Ces providers fonctionnent en renseignant URL + clé API + modèle.

| Provider | Modèle | Prix/min | Latence | WER | Langues | Base URL | Auth |
|----------|--------|----------|---------|-----|---------|----------|------|
| **Groq** | whisper-large-v3-turbo | $0.0007 | <200ms | ~3-4% EN | 99 | `https://api.groq.com/openai/v1` | `Bearer gsk_...` |
| **Groq** | whisper-large-v3 | $0.0019 | <200ms | ~3% EN | 99 | idem | idem |
| **Groq** | distil-whisper | **$0.0003** | <200ms | ~7% EN | EN | idem | idem |
| **Fireworks** | whisper-v3-turbo | $0.0009 | <300ms | ~2% LS | 99 | `https://api.fireworks.ai/inference/v1` | `Bearer fw_...` |
| **Fireworks** | whisper-v3 | $0.0015 | <300ms | ~2% LS | 99 | idem | idem |
| **Together** | whisper-large-v3 | $0.0015 | Sub-sec | ~3% EN | 50+ | `https://api.together.xyz/v1` | `Bearer ...` |
| **OpenAI** | gpt-4o-mini-transcribe | $0.003 | ~320ms | ~2.5% | 99+ | `https://api.openai.com/v1` | `Bearer sk-...` |
| **OpenAI** | gpt-4o-transcribe | $0.006 | ~320ms | 2.46% | 99+ | idem | idem |
| **OpenAI** | whisper-1 | $0.006 | ~500ms | ~4% | 99 | idem | idem |
| **Mistral** | pixtral (multimodal audio) | — | — | — | Multi | `https://api.mistral.ai/v1` | `Bearer ...` |
| **SambaNova** | whisper-large-v3 | — | — | — | 99 | `https://api.sambanova.ai/v1` | `Bearer ...` |

**Recommandation** : Groq whisper-large-v3-turbo — le plus rapide et le moins cher pour du multilingue. Groq distil-whisper = le moins cher toutes catégories ($0.02/heure, EN seul). OpenAI gpt-4o-transcribe = meilleure qualité absolue (90% fewer hallucinations vs Whisper v2).

---

## Cloud ASR — APIs propriétaires (non intégré)

APIs ASR qui ne suivent pas le format OpenAI et nécessitent un `jona-provider-*` dédié.

| Provider | Modèle | Prix/min | Latence | WER | FR | Streaming FR | API | Complexité | Priorité |
|----------|--------|----------|---------|-----|-----|-------------|-----|------------|----------|
| **Deepgram** | Nova-3 | $0.0043 | <300ms | 5.26% | Oui (codeswitching) | Oui | REST sync | **Faible** | ⭐ **Haute** |
| **Rev.ai** | Reverb Foreign | $0.005 | <1s | — | Oui | Oui | REST sync | Faible | Moyenne |
| **AssemblyAI** | Universal-2 | $0.0025 | Secondes/stream | ~14.5% | Oui | Oui (6 langues) | REST + polling | Élevée | Moyenne |
| **ElevenLabs** | Scribe v2 | $0.01 | <150ms stream | ~93.5% FLEURS | Oui | Oui | Multipart sync | Faible | Basse |
| **Gladia** | Whisper-Zero | $0.010 | Temps réel | — | Oui | Oui | Multipart + polling | Moyenne | Basse |
| **Speechmatics** | Flow API | Sur devis | ~150ms | — | Oui (55+ lang) | Oui | REST/WebSocket | Moyenne | Basse |
| **Google Cloud** | Chirp 3 | $0.016 | 1-2s | — | Oui (100+ lang) | Oui | GCP IAM complexe | Très élevée | Très basse |
| **Azure Speech** | Standard | $0.017 | ~200ms | — | Oui (100+ lang) | Oui | SDK Azure | Très élevée | Très basse |
| **Amazon Transcribe** | Standard | $0.024 | Secondes | — | Oui (100+ lang) | Oui | S3 + IAM | Très élevée | Très basse |

### Détails des APIs prioritaires

#### Deepgram

| | Détail |
|--|--------|
| **Endpoint** | `POST https://api.deepgram.com/v1/listen?model=nova-3&language=fr` |
| **Auth** | `Authorization: Token dg_...` (pas Bearer) |
| **Body** | Raw audio bytes (`Content-Type: audio/wav`) |
| **Réponse** | `results.channels[0].alternatives[0].transcript` |
| **Modèles** | nova-3 (flagship), nova-2, enhanced, base |
| **Langues** | 40+ langues, auto-détection, codeswitching |
| **Features** | Smart formatting (ponctuation, nombres), streaming WebSocket, $200 crédits gratuits |
| **Crate** | `jona-provider-deepgram` (~50-80 lignes Rust) |

#### AssemblyAI

| | Détail |
|--|--------|
| **Endpoint** | `POST https://api.assemblyai.com/v2/transcript` |
| **Auth** | `authorization: ...` |
| **Format** | **Asynchrone** 3 étapes : upload audio → create transcript → poll status |
| **Modèles** | best, nano (léger), conformer-2 |
| **Langues** | 99+ langues |
| **Features** | Speaker diarization, sentiment analysis, résumés. Mode real-time streaming aussi disponible. |
| **Crate** | `jona-provider-assemblyai` — complexité élevée (workflow async) |

#### Rev.ai

| | Détail |
|--|--------|
| **Endpoint** | `POST https://api.rev.ai/speechtotext/v1/jobs` |
| **Auth** | `Authorization: Bearer ...` |
| **Format** | REST simple, multipart upload |
| **Langues** | Multi (Reverb Foreign = non-anglais) |
| **Crate** | `jona-provider-revai` (~50 lignes) |

---

## Cloud LLM — OpenAI-compatible (intégré)

L'app supporte `POST /v1/chat/completions` (format OpenAI) pour le cleanup texte.

| Provider | Modèle | Input $/1M | Output $/1M | Vitesse | Free tier | Base URL | Auth |
|----------|--------|-----------|------------|---------|-----------|----------|------|
| **Groq** | llama-3.1-8b-instant | $0.05 | $0.08 | 1200 tok/s | Non | `https://api.groq.com/openai/v1` | `Bearer gsk_...` |
| **Cerebras** | llama3.1-8b | $0.10 | $0.10 | 1800 tok/s | Oui (30 req/min) | `https://api.cerebras.ai/v1` | `Bearer csk-...` |
| **Mistral** | ministral-3b-latest | $0.10 | $0.10 | Rapide | Non | `https://api.mistral.ai/v1` | `Bearer ...` |
| **OpenAI** | gpt-4.1-nano | $0.10 | $0.15 | Rapide | Non | `https://api.openai.com/v1` | `Bearer sk-...` |
| **OpenAI** | gpt-5-nano | $0.05 | $0.40 | Rapide | Non | idem | idem |
| **Google** | gemini-2.5-flash-lite | $0.10 | $0.40 | Rapide | Oui (15 req/min) | `.../v1beta/openai` | `Bearer AIza...` |
| **Together** | Llama-3.2-3B | $0.06 | $0.06 | Modéré | Non | `https://api.together.xyz/v1` | `Bearer ...` |
| **DeepSeek** | deepseek-v3.2 | $0.03 (hit) | $0.42 | Lent TTFT (7.5s) | Non | `https://api.deepseek.com/v1` | `Bearer sk-...` |
| **Fireworks** | llama-v3p1-405b | $3.00 | $3.00 | Modéré | Non | `https://api.fireworks.ai/inference/v1` | `Bearer fw_...` |
| **OpenRouter** | 200+ modèles | Variable | Variable | Variable | Non | `https://openrouter.ai/api/v1` | `Bearer sk-or-...` |
| **xAI** | grok-2 | $2.00 | $10.00 | Rapide | Non | `https://api.x.ai/v1` | `Bearer xai-...` |
| **SambaNova** | Meta-Llama-3.1-8B-Instant | — | — | Très rapide | Oui (rate-limited) | `https://api.sambanova.ai/v1` | `Bearer ...` |
| **Nebius AI** | meta-llama/Meta-Llama-3.1-8B-Instruct | — | — | Rapide | Non | `https://api.studio.nebius.com/v1` | `Bearer ...` |

**Recommandation** : Groq Llama 8B pour la vitesse, GPT-4.1-nano pour la qualité propriétaire. Cerebras/Gemini si free tier souhaité. Éviter DeepSeek (latence trop haute pour du temps réel).

---

## Cloud LLM — APIs propriétaires

### Anthropic (implémenté : `jona-provider-anthropic`)

| | Détail |
|--|--------|
| **Endpoint** | `https://api.anthropic.com/v1/messages` |
| **Auth** | `x-api-key: sk-ant-api03-...` |
| **Format** | Propriétaire (`messages` API avec `role`/`content` blocks) |
| **Modèles** | claude-haiku-4-5 ($1/$5), claude-sonnet-4-5 ($3/$15), claude-opus-4 ($15/$75) |
| **Particularités** | Versioning via header `anthropic-version`, streaming SSE, pas de `/v1/models` standard |

### Google Gemini (ASR natif)

| | Détail |
|--|--------|
| **Endpoint** | `https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent` |
| **Auth** | `?key=AIza...` (query param) ou `Bearer` OAuth |
| **ASR** | Audio en base64 dans `inline_data` (mime `audio/wav`) — API multimodale |
| **Statut** | LLM déjà supporté via le layer OpenAI-compat. ASR natif nécessiterait `jona-provider-gemini`. |

### Cohere

| | Détail |
|--|--------|
| **Endpoint** | `https://api.cohere.com/v2/chat` |
| **Auth** | `Authorization: Bearer ...` |
| **Modèles** | command-r-plus ($2.50/$10), command-r ($0.15/$0.60) |
| **Particularités** | Spécialisé RAG/search. v2 API proche OpenAI mais réponse différente. Pas prioritaire. |

### GitHub Copilot (token exchange)

| | Détail |
|--|--------|
| **Auth initiale** | Token GitHub OAuth `gho_...` (depuis keychain `gh:github.com`) |
| **Token exchange** | `GET https://api.github.com/copilot_internal/v2/token` avec `Authorization: token gho_...` |
| **Token résultat** | JWT court-durée (~30 min) |
| **Endpoint** | `https://api.githubcopilot.com/chat/completions` (format OpenAI) |
| **Headers requis** | `Editor-Version`, `Copilot-Integration-Id`, `Openai-Organization: github-copilot` |
| **Modèles** | GPT-4o, Claude 3.5 Sonnet (choix limité par Copilot) |
| **Prix** | Inclus dans l'abonnement Copilot ($10–$39/mois) |
| **Particularités** | API reverse-engineered. Réfs : `copilot-api`, `copilot-proxy`, LiteLLM `github_copilot/`. |
| **Crate** | `jona-provider-copilot` — token exchange + requêtes OpenAI-format |

### Non prioritaires

| Provider | Raison |
|----------|--------|
| **Perplexity** | Search-focused (`pplx-...`), pas pertinent pour correction texte |
| **Replicate** | API async, pas de modèles propres, complexité élevée |
| **JetBrains AI** | Pas d'API publique documentée (la clé Anthropic saisie dans l'IDE est exploitable via keychain — voir AUTO-DETECTION.md) |
| **AWS Bedrock** | Auth SigV4 complexe, niche enterprise |
| **Azure OpenAI** | Deployment-based, niche enterprise |

---

## Self-hosted — Serveurs ASR

Pour les utilisateurs qui veulent héberger leur propre serveur ASR. Tous utilisent le format OpenAI (`/v1/audio/transcriptions`).

| Solution | API OpenAI | Mac natif | Langues | Install | Performance | Statut |
|----------|-----------|-----------|---------|---------|-------------|--------|
| **whisper.cpp server** | Oui | Metal + CoreML | 99 | `brew install whisper-cpp` | 27x RTF (tiny M4) | **Supporté** (Custom) |
| **faster-whisper** | Oui (wrapper) | CPU | 99 | `pip install faster-whisper` | 4x Whisper (CTranslate2) | **Supporté** (Custom) |
| **MLX-Audio** | Oui | MLX Apple Silicon | 99 | `pip install mlx-audio` | 10x whisper.cpp | **Supporté** (Custom) |
| **MLX-Whisper** | Oui | MLX Apple Silicon | 99 | `pip install mlx-whisper` | 2x Whisper long audio | **Supporté** (Custom) |
| **Speaches** | Oui | Docker | 99 | Docker one-liner | 4x Whisper | **Supporté** (Custom) |
| **LocalAI** | Oui | Docker | 99 | Docker + config YAML | Variable | **Supporté** (Custom) |
| whisper-jax | Via wrapper | CPU/TPU | 99 | `pip install whisper-jax` | 70x Whisper (JAX) | Non supporté — pas d'API OpenAI native |
| sherpa-onnx server | Via C API | Oui | Multi | pip/build | Très rapide | Non supporté — pas d'API OpenAI |
| Vosk | Non (WebSocket) | Oui | 20+ | pip/Docker | Temps réel | Non supporté — API WebSocket |

## Self-hosted — Serveurs LLM

Tous exposent `/v1/chat/completions` compatible OpenAI.

| Serveur | Port | Install macOS | Metal GPU | Facilité (1-5) | Statut |
|---------|------|---------------|-----------|----------------|--------|
| **Ollama** | 11434 | `brew install ollama` | Oui | 5 | **Supporté** (Custom) |
| **LM Studio** | 1234 | .dmg GUI | Metal + MLX | 5 | **Supporté** (Custom) |
| **llama-server** | 8080 | `brew install llama.cpp` | Oui | 3 | **Supporté** (Custom) |
| **Jan.ai** | 1337 | .dmg GUI | Oui | 4 | **Supporté** (Custom) |
| **GPT4All** | 4891 | .dmg GUI | Oui | 4 | **Supporté** (Custom) |
| **llamafile** | 8080 | Fichier unique | Oui | 5 | **Supporté** (Custom) |
| **LocalAI** | 8080 | Docker | Docker | 2 | **Supporté** (Custom) |
| **KoboldCpp** | 5001 | Exécutable | Oui | 3 | **Supporté** (Custom) |

---

## Presets actuels

12 providers OpenAI-compatible préconfigurés dans le crate `jona-provider-openai` (système inventory) + 1 provider Anthropic dans le crate `jona-provider-anthropic`. L'utilisateur entre juste sa clé API.

| Provider | `id` | ASR | LLM | Base URL |
|----------|------|-----|-----|----------|
| **OpenAI** | `openai` | whisper-1, gpt-4o-transcribe, gpt-4o-mini-transcribe | gpt-4o-mini, gpt-4o | `api.openai.com` |
| **Groq** | `groq` | whisper-large-v3-turbo, whisper-large-v3 | llama-3.1-8b-instant | `api.groq.com` |
| **Cerebras** | `cerebras` | — | llama3.1-8b | `api.cerebras.ai` |
| **Google Gemini** | `gemini` | — | gemini-2.5-flash-lite | `generativelanguage.googleapis.com` |
| **Mistral** | `mistral` | — | ministral-3b-latest | `api.mistral.ai` |
| **Fireworks** | `fireworks` | whisper-v3-turbo, whisper-v3 | — | `api.fireworks.ai` |
| **Together** | `together` | openai/whisper-large-v3 | Llama-3.2-3B | `api.together.xyz` |
| **DeepSeek** | `deepseek` | — | deepseek-v3.2 | `api.deepseek.com` |
| **OpenRouter** | `openrouter` | — | (200+ modèles) | `openrouter.ai` |
| **xAI** | `xai` | — | grok-2 | `api.x.ai` |
| **SambaNova** | `sambanova` | whisper-large-v3 | Meta-Llama-3.1-8B-Instant | `api.sambanova.ai` |
| **Nebius AI** | `nebius` | — | meta-llama/Meta-Llama-3.1-8B-Instruct | `api.studio.nebius.com` |
| **Anthropic** | `anthropic` | — | claude-haiku-4-5, claude-sonnet-4-5, claude-opus-4-6 | `api.anthropic.com` |
| **Deepgram** | `deepgram` | nova-3, nova-2 | — | `api.deepgram.com` |
| **GitHub Copilot** | `copilot` | — | gpt-4o, gpt-4o-mini | `api.githubcopilot.com` |
| **Gemini ASR** | `gemini-asr` | gemini-2.0-flash, gemini-2.5-flash | — | `generativelanguage.googleapis.com` |
| **Rev.ai** | `revai` | reverb-english, reverb-foreign | — | `api.rev.ai` |
| **AssemblyAI** | `assemblyai` | best, nano | — | `api.assemblyai.com` |
| **ElevenLabs** | `elevenlabs` | scribe_v2, scribe_v1 | — | `api.elevenlabs.io` |
| **Cohere** | `cohere` | — | command-r-plus, command-r | `api.cohere.com` |

---

## Résumé décisionnel

### Providers dédiés (crate `jona-provider-*` — tous implémentés ✅)

| Provider | Crate | ASR | LLM | Status |
|----------|-------|-----|-----|--------|
| **Deepgram** | `jona-provider-deepgram` | ✅ | ❌ | ✅ Implémenté |
| **GitHub Copilot** | `jona-provider-copilot` | ❌ | ✅ | ✅ Implémenté |
| **Gemini ASR** | `jona-provider-gemini-asr` | ✅ | ❌ | ✅ Implémenté |
| **Rev.ai** | `jona-provider-revai` | ✅ | ❌ | ✅ Implémenté |
| **AssemblyAI** | `jona-provider-assemblyai` | ✅ | ❌ | ✅ Implémenté |
| **ElevenLabs** | `jona-provider-elevenlabs` | ✅ | ❌ | ✅ Implémenté |
| **Cohere** | `jona-provider-cohere` | ❌ | ✅ | ✅ Implémenté |

---

*Dernière mise à jour : mars 2026*
