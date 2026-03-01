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
| **sherpa-onnx** | Via C API | Oui | Oui (multi-modèles) | Build from source / pip | Très rapide | Supporte Whisper, Canary, Parakeet, SenseVoice |
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

### Modèles ASR non-Whisper (recherche avancée)

Les modèles non-Whisper dominent le Open ASR Leaderboard. Certains sont intégrables via ONNX ou même GGML.

| Modèle | Params | FR | Format | Taille | Intérêt | Intégration Rust |
|---|---|---|---|---|---|---|
| **bofenghuang/whisper-large-v3-french** | 1.5B | Natif FR | **GGML dispo** | ~3 GB | Meilleur Whisper FR, fine-tuné sur données françaises | Compatible whisper-rs existant ! |
| **NVIDIA Canary-180M-Flash** | 182M | Oui (52 langues) | ONNX | ~360 MB | Ultra-léger, bat Whisper Medium | Via sherpa-onnx C API ou `canary-rs` |
| **Qwen3-ASR** | 0.6B / 1.7B | Oui (52 langues) | PyTorch | ~1.2 / 3.4 GB | Bat Whisper sur benchmarks, Alibaba | Pas encore GGML/ONNX, à surveiller |
| **SenseVoice Small** | 234M | Oui (50+ langues) | ONNX | ~450 MB | Alibaba, très rapide | Via sherpa-onnx |
| **Moonshine** | 27M / 61M | EN seul | ONNX | ~50 / 120 MB | Ultra-léger, temps réel sur edge | Via sherpa-onnx |
| **Parakeet-TDT v3** | 1.1B | Oui (25 langues) | ONNX | ~2 GB | Meilleur WER absolu | Via sherpa-onnx ou NeMo |

**Écosystème Rust** :
- **sherpa-onnx** : C API avec bindings Rust, supporte Canary, Parakeet, SenseVoice, Moonshine, Whisper
- **transcribe-rs** : Crate Rust abstrayant plusieurs backends ASR
- **canary-rs** : Bindings Rust spécifiques pour NVIDIA Canary

**Candidat prioritaire** : `bofenghuang/whisper-large-v3-french` — compatible avec notre whisper-rs existant (format GGML), meilleur WER français. Juste une entrée catalogue à ajouter.

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

### Modèles de ponctuation candidats

Alternatives ou compléments au BERT fullstop actuel.

| Modèle | Architecture | Taille | Langues | ONNX | Intérêt |
|---|---|---|---|---|---|
| **1-800-BAD-CODE/punct_cap_seg_47_language** | Transformer 6L d=512 | ~200 MB | 47 langues (FR, EN) | Exportable | Ponctuation + capitalisation + segmentation en un pass. F1=97.39 |
| **sherpa-onnx-online-punct-en** | CNN-BiLSTM | **7.1 MB** (int8) | EN | Oui | Ultra-léger, 1/40e la taille de BERT, 2.5x plus rapide |
| **oliverguhr/fullstop-multilingual-sonar-base** | XLM-RoBERTa Base | ~500 MB | FR, EN, DE, IT, NL | Exportable | Version Base du modèle actuel, plus petit |

### Modèles de correction spécialisés (alternative au LLM)

Approche pipeline : chaîner des modèles légers spécialisés au lieu d'un LLM généraliste.

| Modèle | Architecture | Params | Langues | Tâche | Intérêt |
|---|---|---|---|---|---|
| **FlanEC** (morenolq/flanec-large-cd) | Flan-T5 Base/Large | 250M-800M | EN | Post-ASR error correction | Seul modèle conçu spécifiquement pour corriger les erreurs ASR |
| **fdemelo/t5-base-spell-correction-fr** | T5-Base | 220M | **FR** | Correction orthographe + ponctuation | Entraîné sur corpus FR, licence MIT |
| **Unbabel/gec-t5_small** | T5-Small | 60M | Multilingue | Grammar error correction | Le plus petit T5 GEC, multilingue |
| **pszemraj/flan-t5-large-grammar-synthesis** | Flan-T5-Large | 783M | EN | Grammar correction | ONNX + **GGUF** disponibles, conçu pour ASR |
| **sdadas/byt5-text-correction** | ByT5 | 300M | 102+ langues | Correction character-level | Robuste aux erreurs ASR (pas de tokenizer) |
| **Harper** (crate Rust) | Rule-based | N/A | EN | Grammar checking | <10ms, pure Rust, offline |

**Approche pipeline recommandée** (alternative au LLM, <50ms total) :
1. Regex filler words ("euh", "uh", "um", "hein", "you know") — ~0ms
2. Token classification ponctuation (7-200 MB ONNX) — ~10-30ms
3. Grammar légère : Harper (EN) / règles regex (FR) — ~5ms

---

## Audio — Denoising & VAD

### Constat critique : le denoising dégrade Whisper

Le paper **"When De-noising Hurts"** (arXiv:2512.17562, déc 2025) démontre que le speech enhancement **dégrade** les performances ASR dans **toutes** les configurations testées (40/40), avec des augmentations de WER de 1.1% à 46.6%. Whisper est entraîné sur 680K heures d'audio bruité et possède une robustesse interne au bruit. Le denoising introduit des artefacts spectraux plus nuisibles que le bruit original.

**Ce qui aide Whisper** : le VAD (trimming silence) pour éviter les hallucinations.

Voir `docs/AUDIO-PIPELINE.md` pour l'architecture complète.

### VAD (Voice Activity Detection)

| Solution | Type | Taille | Latence | Précision | Rust | Note |
|---|---|---|---|---|---|---|
| **Silero VAD v6** | ML (ONNX) | 2 MB | <1ms / 30ms chunk | 87.7% TPR @ 5% FPR | `voice_activity_detector`, `silero-vad-rust` | **Recommandé** — 4x moins d'erreurs que WebRTC VAD |
| **TEN VAD** | ML (ONNX) | ~1 MB | Ultra-faible | Meilleur que Silero sur transitions | ONNX via `ort` | Transition parole→silence plus rapide |
| **WebRTC VAD** | GMM | ~10 KB | ~0ms | 50% TPR @ 5% FPR | `webrtc-vad` crate | Simple mais 2x moins précis que Silero |
| **NVIDIA MarbleNet v2** | CNN | ~400 KB | <1ms / 20ms frame | Bon | ONNX exportable | 91.5K params, frame-level, multilingue |
| **nnnoiseless VAD** | RNNoise | ~85 KB | <1ms / 10ms frame | Correct | Natif (inclus) | Gratuit si on utilise nnnoiseless pour denoising |

### Denoising (optionnel, configurable)

| Solution | Type | Taille | RTF (CPU) | PESQ | Rust | Note |
|---|---|---|---|---|---|---|
| **nnnoiseless** | Pure Rust | 85 KB | <<0.01 | ~3.88 | Natif | Port RNNoise, zéro dépendance, le plus simple |
| **DeepFilterNet3** | Rust + tract | ~8 MB | 0.04 | 3.5+ | `deep_filter` crate | Meilleure qualité, pipeline Rust natif via tract |
| **DTLN** | ONNX | ~2 MB | <0.01 | ~2.7 | `dtln-rs` (Datadog) | Rust+WASM, 33ms/s sur M1 |
| **GTCRN** | ONNX | <100 KB | <<0.01 | 2.87 | ONNX via `ort` | **23.7K params** — le plus léger existant |
| **UL-UNAS** | ONNX | ~500 KB | Très rapide | 3.09 | ONNX via `ort` | 169K params, meilleur ratio qualité/taille |

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
