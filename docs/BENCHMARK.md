# Benchmark ASR & LLM — Mars 2026

Référence complète des options ASR (speech-to-text) et LLM (text cleanup) disponibles pour WhisperDictate. Couvre le cloud, le self-hosted, et le natif intégré.

---

## ASR — Speech-to-Text

### Cloud — Compatible OpenAI (intégré, zéro code)

L'app supporte déjà `POST /v1/audio/transcriptions` avec multipart form. Ces providers fonctionnent en renseignant URL + clé API + modèle dans un provider Custom.

| Provider | Modèle | Prix/min | Latence | WER | Free tier | URL de base |
|---|---|---|---|---|---|---|
| **Groq** | whisper-large-v3-turbo | $0.0007 | <200ms | ~3-4% EN | Non | `https://api.groq.com/openai/v1` |
| **Groq** | whisper-large-v3 | $0.0019 | <200ms | ~3% EN | Non | idem |
| **Fireworks AI** | whisper-v3-turbo | $0.0009 | <300ms | ~2% LS | Non | `https://api.fireworks.ai/inference/v1` |
| **Fireworks AI** | whisper-v3 | $0.0015 | <300ms | ~2% LS | Non | idem |
| **Together AI** | openai/whisper-large-v3 | $0.0015 | Sub-sec | ~3% EN | Non | `https://api.together.xyz/v1` |
| **OpenAI** | gpt-4o-mini-transcribe | $0.003 | ~320ms | ~2.5% | Non | `https://api.openai.com/v1` |
| **OpenAI** | gpt-4o-transcribe | $0.006 | ~320ms | 2.46% | Non | idem |

**Recommandation** : Groq whisper-large-v3-turbo — le plus rapide et le moins cher. OpenAI gpt-4o-transcribe pour la meilleure qualité absolue.

### Cloud — API propriétaire (nécessite intégration dédiée)

| Provider | Modèle | Prix/min | Latence | Qualité | Difficulté intégration |
|---|---|---|---|---|---|
| **Deepgram** | Nova-3 | $0.004 | <300ms | Excellent (audio bruité) | Moyenne — REST simple, format différent |
| **ElevenLabs** | Scribe v2 | $0.006 | 150ms streaming | Bon | Moyenne — REST propriétaire |
| **Gladia** | Whisper-Zero | $0.010 | Temps réel | Bon (anti-hallucination) | Moyenne — REST propriétaire |
| **AssemblyAI** | Universal-2 | $0.0025 | Secondes (async) | Bon | Moyenne — polling asynchrone |
| **Google Cloud** | Chirp 3 | $0.016 | 1-2s | Excellent | Élevée — GCP IAM, service accounts |
| **Azure Speech** | Standard | $0.006-0.011 | ~200ms | Excellent | Élevée — subscription Azure, SDK |
| **Amazon Transcribe** | Standard | $0.024 | Secondes (S3) | Bon | Très élevée — S3 + IAM + SDK |

### Local / Self-hosted — Serveurs ASR

Pour les utilisateurs qui veulent héberger leur propre serveur ASR.

| Solution | API OpenAI | Mac natif | FR/EN | Install | Performance | Note |
|---|---|---|---|---|---|---|
| **whisper.cpp server** | Oui | Metal + CoreML | Oui (99 langues) | `brew install whisper-cpp` | 27x temps réel (tiny M4) | **Top pick** |
| **MLX-Audio** | Oui | MLX Apple Silicon | Oui | `pip install mlx-audio` | 10x whisper.cpp sur Mac | Nécessite Python |
| **Speaches** | Oui | Docker (pas Metal) | Oui | Docker one-liner | 4x Whisper original | Bon pour Linux/NAS |
| **LocalAI** | Oui | Docker | Oui | Docker + config YAML | Variable | Swiss army knife |
| **Parakeet-TDT v3** | Via wrappers | CPU ONNX | Oui (25 langues) | Python/Go wrappers | 3300x temps réel (GPU) | Meilleure précision, GPU NVIDIA idéal |
| **OWhisper** | À vérifier | Oui | Oui | Binaire pré-compilé | Streaming VAD | "Ollama for STT", jeune projet |
| **Vosk** | Non (WebSocket) | Oui | Oui | pip/Docker | Temps réel | Léger mais qualité inférieure |

### Natif intégré (whisper-rs, dans l'app)

Modèles GGML téléchargeables depuis le Model Manager, exécutés en local via whisper-rs + Metal GPU.

| Modèle | ID | Taille | WER | RTF | Recommandé |
|---|---|---|---|---|---|
| Large V3 | `whisper:large-v3` | 3.1 GB | 1.8% | 0.50 | |
| Large V2 | `whisper:large-v2` | 3.09 GB | 1.9% | 0.50 | |
| **Large V3 Turbo** | `whisper:large-v3-turbo` | 1.6 GB | 2.1% | 0.25 | **Recommandé** |
| Large V3 Turbo Q8 | `whisper:large-v3-turbo-q8` | 874 MB | 2.1% | 0.20 | |
| Large V3 Turbo Q5 | `whisper:large-v3-turbo-q5` | 574 MB | 2.3% | 0.15 | |
| Medium | `whisper:medium` | 1.5 GB | 2.7% | 0.35 | |
| Medium Q5 | `whisper:medium-q5` | 539 MB | 2.8% | 0.20 | |
| Small | `whisper:small` | 466 MB | 3.4% | 0.15 | |
| Small Q5 | `whisper:small-q5` | 190 MB | 3.6% | 0.10 | |
| Base | `whisper:base` | 142 MB | 5.0% | 0.08 | |
| Tiny | `whisper:tiny` | 75 MB | 7.6% | 0.05 | |

---

## LLM — Text Cleanup

### Cloud — Compatible OpenAI (intégré, zéro code)

L'app supporte `POST /v1/chat/completions` (OpenAI-compatible) et le format Anthropic Messages.

| Provider | Modèle | Input $/1M | Output $/1M | Vitesse | Free tier | URL de base |
|---|---|---|---|---|---|---|
| **Groq** | llama-3.1-8b-instant | $0.05 | $0.08 | 1200 tok/s | Non | `https://api.groq.com/openai/v1` |
| **Cerebras** | llama3.1-8b | $0.10 | $0.10 | 1800 tok/s | Oui (30 req/min) | `https://api.cerebras.ai/v1` |
| **Mistral** | mistral-nemo | $0.02 | $0.02 | Modéré | Non | `https://api.mistral.ai/v1` |
| **Mistral** | ministral-3b-latest | $0.10 | $0.10 | Rapide | Non | idem |
| **OpenAI** | gpt-4.1-nano | $0.10 | $0.15 | Rapide | Non | `https://api.openai.com/v1` |
| **OpenAI** | gpt-5-nano | $0.05 | $0.40 | Rapide | Non | idem |
| **Google** | gemini-2.5-flash-lite | $0.10 | $0.40 | Rapide | Oui (15 req/min) | `https://generativelanguage.googleapis.com/v1beta/openai` |
| **Together AI** | meta-llama/Llama-3.2-3B | $0.06 | $0.06 | Modéré | Non | `https://api.together.xyz/v1` |
| **DeepSeek** | deepseek-v3.2 | $0.03 (hit) | $0.42 | Lent TTFT (7.5s) | Non | `https://api.deepseek.com/v1` |
| **Anthropic** | claude-haiku-4-5 | $1.00 | $5.00 | 110 tok/s | Non | `https://api.anthropic.com` (format Anthropic) |

**Recommandation** : Groq Llama 8B pour la vitesse, GPT-4.1-nano pour la qualité propriétaire. Cerebras/Gemini si free tier souhaité. Éviter DeepSeek (latence trop haute pour du temps réel).

### Local / Self-hosted — Serveurs LLM

Tous exposent `/v1/chat/completions` compatible OpenAI.

| Serveur | Port par défaut | Install macOS | Metal GPU | Facilité (1-5) | Note |
|---|---|---|---|---|---|
| **Ollama** | 11434 | `brew install ollama` | Oui | 5 | **Top pick** — service background, immense catalogue |
| **LM Studio** | 1234 | .dmg GUI | Metal + MLX | 5 | Meilleur pour les non-techniques, MLX 20-30% plus rapide |
| **llama-server** | 8080 | `brew install llama.cpp` | Oui | 3 | Power users, même moteur qu'Ollama |
| **Jan.ai** | 1337 | .dmg GUI | Oui | 4 | Interface ChatGPT-like |
| **GPT4All** | 4891 | .dmg GUI | Oui | 4 | Catalogue limité, API secondaire |
| **llamafile** | 8080 | Fichier unique | Oui | 5 | Zéro install mais un fichier par modèle |
| **LocalAI** | 8080 | Docker | Docker | 2 | Pour homelab/serveurs |
| **KoboldCpp** | 5001 | Exécutable | Oui | 3 | Niche creative writing |

### Natif intégré (llama-cpp-2, dans l'app)

Modèles GGUF téléchargeables depuis le Model Manager, exécutés en local via llama-cpp-2 + Metal GPU.

| Modèle | ID | Taille | Params | RAM | FR/EN | Recommandé |
|---|---|---|---|---|---|---|
| **Qwen3 1.7B** | `llama:qwen3-1.7b` | 1.28 GB | 1.7B | 1.5 GB | Oui | **Recommandé** |
| Qwen3 4B | `llama:qwen3-4b` | 2.5 GB | 4B | 3 GB | Oui | |
| SmolLM2 1.7B | `llama:smollm2-1.7b` | 1.06 GB | 1.7B | 1.3 GB | EN seul | |
| Gemma 3 1B | `llama:gemma3-1b` | 806 MB | 1B | 1 GB | Oui | |
| Phi-4 Mini | `llama:phi4-mini` | 2.49 GB | 3.8B | 3 GB | EN seul | |

### Modèles candidats à ajouter au catalogue natif

Ces modèles GGUF tournent directement via llama-cpp-2 sans code supplémentaire — juste une entrée dans le catalogue.

| Modèle | Taille Q4 | Params | FR/EN | Intérêt | Source |
|---|---|---|---|---|---|
| **Qwen3 0.6B** | ~400 MB | 0.6B | Oui | Ultra rapide (80-120 tok/s M1) | bartowski/Qwen_Qwen3-0.6B-GGUF |
| **Gemma 3 4B** | ~2.5 GB | 4B | Oui | Alternative à Qwen3 4B, 140+ langues | bartowski/google_gemma-3-4b-it-GGUF |
| **SmolLM3 3B** | ~1.8 GB | 3B | Partiel | Bat Llama 3.2 3B et Qwen2.5 3B | bartowski/SmolLM3-3B-Instruct-GGUF |
| **Llama 3.2 1B** | ~700 MB | 1B | Partiel | Meta, bon en summarization | bartowski/Llama-3.2-1B-Instruct-GGUF |
| **Llama 3.2 3B** | ~1.8 GB | 3B | Partiel | Meta, bat Gemma 2 2.6B | bartowski/Llama-3.2-3B-Instruct-GGUF |
| **Ministral 3B** | ~1.8 GB | 3B | Oui | Mistral, bon en FR | bartowski/Ministral-3B-Instruct-GGUF |

### BERT Punctuation (natif intégré)

| Modèle | ID | Taille | RAM | Langues | Vitesse |
|---|---|---|---|---|---|
| Fullstop Multilang Large INT8 | `bert-punctuation:fullstop-multilang-large` | 562 MB | 600 MB | FR, EN, DE, IT | ~100ms |

---

## Comparaison des approches

| Approche | Latence | Coût | Qualité | Offline | Setup utilisateur |
|---|---|---|---|---|---|
| **Natif intégré** | Très faible | Gratuit | Bon → Excellent | Oui | Télécharger le modèle |
| **Cloud OpenAI-compat** | Faible | $0.0007-0.006/min | Excellent | Non | URL + clé API |
| **Cloud propriétaire** | Variable | $0.003-0.024/min | Excellent | Non | Intégration dédiée |
| **Self-hosted** | Faible (LAN) | Gratuit (hardware) | Bon → Excellent | Oui (LAN) | Installer serveur |

---

*Dernière mise à jour : mars 2026*
