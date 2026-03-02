# Benchmark ASR & LLM — Mars 2026

Référence complète des options ASR (speech-to-text) et LLM (text cleanup) disponibles pour WhisperDictate. Couvre le cloud, le self-hosted, et le natif intégré.

---

## ASR — Speech-to-Text

### Cloud — Compatible OpenAI (intégré, zéro code)

L'app supporte déjà `POST /v1/audio/transcriptions` avec multipart form. Ces providers fonctionnent en renseignant URL + clé API + modèle dans un provider Custom.

| Provider | Modèle | Prix/min | Latence | WER | Free tier | URL de base | Statut |
|---|---|---|---|---|---|---|---|
| **Groq** | whisper-large-v3-turbo | $0.0007 | <200ms | ~3-4% EN | Non | `https://api.groq.com/openai/v1` | **Supporté** — via provider custom |
| **Groq** | whisper-large-v3 | $0.0019 | <200ms | ~3% EN | Non | idem | **Supporté** — via provider custom |
| **Fireworks AI** | whisper-v3-turbo | $0.0009 | <300ms | ~2% LS | Non | `https://api.fireworks.ai/inference/v1` | **Supporté** — via provider custom |
| **Fireworks AI** | whisper-v3 | $0.0015 | <300ms | ~2% LS | Non | idem | **Supporté** — via provider custom |
| **Together AI** | openai/whisper-large-v3 | $0.0015 | Sub-sec | ~3% EN | Non | `https://api.together.xyz/v1` | **Supporté** — via provider custom |
| **OpenAI** | gpt-4o-mini-transcribe | $0.003 | ~320ms | ~2.5% | Non | `https://api.openai.com/v1` | **Supporté** — via provider custom |
| **OpenAI** | gpt-4o-transcribe | $0.006 | ~320ms | 2.46% | Non | idem | **Supporté** — via provider custom |

**Recommandation** : Groq whisper-large-v3-turbo — le plus rapide et le moins cher. OpenAI gpt-4o-transcribe pour la meilleure qualité absolue.

### Cloud — API propriétaire (nécessite intégration dédiée)

| Provider | Modèle | Prix/min | Latence | Qualité | Difficulté intégration | Statut |
|---|---|---|---|---|---|---|
| **Deepgram** | Nova-3 | $0.004 | <300ms | Excellent (audio bruité) | Moyenne — REST simple, format différent | Non intégré — format réponse incompatible OpenAI |
| **ElevenLabs** | Scribe v2 | $0.006 | 150ms streaming | Bon | Moyenne — REST propriétaire | Non intégré — API propriétaire |
| **Gladia** | Whisper-Zero | $0.010 | Temps réel | Bon (anti-hallucination) | Moyenne — REST propriétaire | Non intégré — API propriétaire |
| **AssemblyAI** | Universal-2 | $0.0025 | Secondes (async) | Bon | Moyenne — polling asynchrone | Non intégré — modèle async incompatible temps réel |
| **Google Cloud** | Chirp 3 | $0.016 | 1-2s | Excellent | Élevée — GCP IAM, service accounts | Non intégré — complexité d'auth trop élevée |
| **Azure Speech** | Standard | $0.006-0.011 | ~200ms | Excellent | Élevée — subscription Azure, SDK | Non intégré — dépendance SDK Azure |
| **Amazon Transcribe** | Standard | $0.024 | Secondes (S3) | Bon | Très élevée — S3 + IAM + SDK | Non intégré — architecture S3 inadaptée |

### Local / Self-hosted — Serveurs ASR

Pour les utilisateurs qui veulent héberger leur propre serveur ASR.

| Solution | API OpenAI | Mac natif | FR/EN | Install | Performance | Statut |
|---|---|---|---|---|---|---|
| **whisper.cpp server** | Oui | Metal + CoreML | Oui (99 langues) | `brew install whisper-cpp` | 27x temps réel (tiny M4) | **Supporté** — via provider custom |
| **MLX-Audio** | Oui | MLX Apple Silicon | Oui | `pip install mlx-audio` | 10x whisper.cpp sur Mac | **Supporté** — via provider custom |
| **sherpa-onnx** | Via C API | Oui | Oui (multi-modèles) | Build from source / pip | Très rapide | Non supporté — pas d'API OpenAI |
| **Speaches** | Oui | Docker (pas Metal) | Oui | Docker one-liner | 4x Whisper original | **Supporté** — via provider custom |
| **LocalAI** | Oui | Docker | Oui | Docker + config YAML | Variable | **Supporté** — via provider custom |
| **Parakeet-TDT v3** | Via wrappers | CPU ONNX | Oui (25 langues) | Python/Go wrappers | 3300x temps réel (GPU) | Non supporté — pas d'API OpenAI (modèle intégré nativement) |
| **OWhisper** | À vérifier | Oui | Oui | Binaire pré-compilé | Streaming VAD | Non vérifié — projet trop jeune |
| **Vosk** | Non (WebSocket) | Oui | Oui | pip/Docker | Temps réel | Non supporté — API WebSocket incompatible |

### Natif intégré (whisper-rs, dans l'app)

Modèles GGML téléchargeables depuis le Model Manager, exécutés en local via whisper-rs + Metal GPU.

| Modèle | ID | Taille | RAM | WER | RTF | Recommandé | Statut |
|---|---|---|---|---|---|---|---|
| Large V3 | `whisper:large-v3` | 3.1 GB | 4 GB | 1.8% | 0.50 | | **Intégré** |
| Large V2 | `whisper:large-v2` | 3.09 GB | 4 GB | 1.9% | 0.50 | | **Intégré** |
| **Large V3 Turbo** | `whisper:large-v3-turbo` | 1.6 GB | 2.5 GB | 2.1% | 0.25 | **Recommandé** | **Intégré** |
| Large V3 Turbo Q8 | `whisper:large-v3-turbo-q8` | 874 MB | 1.3 GB | 2.1% | 0.20 | | **Intégré** |
| Large V3 Turbo Q5 | `whisper:large-v3-turbo-q5` | 574 MB | 900 MB | 2.3% | 0.15 | | **Intégré** |
| Medium | `whisper:medium` | 1.5 GB | 2 GB | 2.7% | 0.35 | | **Intégré** |
| Medium Q5 | `whisper:medium-q5` | 539 MB | 900 MB | 2.8% | 0.20 | | **Intégré** |
| Small | `whisper:small` | 466 MB | 750 MB | 3.4% | 0.15 | | **Intégré** |
| Small Q5 | `whisper:small-q5` | 190 MB | 400 MB | 3.6% | 0.10 | | **Intégré** |
| Base | `whisper:base` | 142 MB | 300 MB | 5.0% | 0.08 | | **Intégré** |
| Tiny | `whisper:tiny` | 75 MB | 200 MB | 7.6% | 0.05 | | **Intégré** |

### Modèles ASR non-Whisper (recherche avancée)

Les modèles non-Whisper dominent le Open ASR Leaderboard. Certains sont intégrables via ONNX ou même GGML.

| Modèle | Params | FR | Format | Taille | RAM | Intérêt | Statut |
|---|---|---|---|---|---|---|---|
| **bofenghuang/whisper-large-v3-french** | 1.5B | Natif FR | GGML | ~538 MB | 900 MB | Meilleur Whisper FR, fine-tuné sur données françaises | **Intégré** (`whisper:large-v3-french-distil`) |
| **NVIDIA Canary-180M-Flash** | 182M | Oui (4 langues) | ONNX int8 | ~214 MB | 300 MB | Ultra-léger, bat Whisper Medium, CoreML GPU | **Intégré** (`canary:180m-flash-int8`) |
| **Parakeet-TDT 0.6B v3** | 600M | Oui (25 langues) | ONNX int8 | ~670 MB | 750 MB | Excellent WER, TDT transducer, CoreML GPU | **Intégré** (`parakeet:tdt-0.6b-v3-int8`) |
| **Qwen3-ASR 0.6B** | 600M | Oui (30 langues) | Safetensors | ~1.88 GB | 2 GB | Bat Whisper, Accelerate/AMX | **Intégré** (`qwen-asr:0.6b`) |
| **SenseVoice Small** | 234M | Oui (5 langues : zh/yue/en/ja/ko) | ONNX | ~228 MB | — | Alibaba, très rapide | Écarté — pas de français |
| **Moonshine** | 27M / 61M | EN seul | ONNX | ~27 / 120 MB | — | Ultra-léger, temps réel sur edge | Écarté — pas de français |

**Écosystème Rust** :
- **sherpa-onnx** : C API avec bindings Rust, supporte Canary, Parakeet, SenseVoice, Moonshine, Whisper
- **transcribe-rs** : Crate Rust abstrayant plusieurs backends ASR
- **canary-rs** : Bindings Rust spécifiques pour NVIDIA Canary

**Candidat prioritaire** : `bofenghuang/whisper-large-v3-french` — compatible avec notre whisper-rs existant (format GGML), meilleur WER français. **Intégré** comme `whisper:large-v3-french-distil`.

---

## LLM — Text Cleanup

### Cloud — Compatible OpenAI (intégré, zéro code)

L'app supporte `POST /v1/chat/completions` (OpenAI-compatible) et le format Anthropic Messages.

| Provider | Modèle | Input $/1M | Output $/1M | Vitesse | Free tier | URL de base | Statut |
|---|---|---|---|---|---|---|---|
| **Groq** | llama-3.1-8b-instant | $0.05 | $0.08 | 1200 tok/s | Non | `https://api.groq.com/openai/v1` | **Supporté** — via provider custom |
| **Cerebras** | llama3.1-8b | $0.10 | $0.10 | 1800 tok/s | Oui (30 req/min) | `https://api.cerebras.ai/v1` | **Supporté** — via provider custom |
| **Mistral** | mistral-nemo | $0.02 | $0.02 | Modéré | Non | `https://api.mistral.ai/v1` | **Supporté** — via provider custom |
| **Mistral** | ministral-3b-latest | $0.10 | $0.10 | Rapide | Non | idem | **Supporté** — via provider custom |
| **OpenAI** | gpt-4.1-nano | $0.10 | $0.15 | Rapide | Non | `https://api.openai.com/v1` | **Supporté** — via provider custom |
| **OpenAI** | gpt-5-nano | $0.05 | $0.40 | Rapide | Non | idem | **Supporté** — via provider custom |
| **Google** | gemini-2.5-flash-lite | $0.10 | $0.40 | Rapide | Oui (15 req/min) | `https://generativelanguage.googleapis.com/v1beta/openai` | **Supporté** — via provider custom |
| **Together AI** | meta-llama/Llama-3.2-3B | $0.06 | $0.06 | Modéré | Non | `https://api.together.xyz/v1` | **Supporté** — via provider custom |
| **DeepSeek** | deepseek-v3.2 | $0.03 (hit) | $0.42 | Lent TTFT (7.5s) | Non | `https://api.deepseek.com/v1` | **Supporté** — via provider custom |
| **Anthropic** | claude-haiku-4-5 | $1.00 | $5.00 | 110 tok/s | Non | `https://api.anthropic.com` (format Anthropic) | **Supporté** — via provider Anthropic |

**Recommandation** : Groq Llama 8B pour la vitesse, GPT-4.1-nano pour la qualité propriétaire. Cerebras/Gemini si free tier souhaité. Éviter DeepSeek (latence trop haute pour du temps réel).

### Local / Self-hosted — Serveurs LLM

Tous exposent `/v1/chat/completions` compatible OpenAI.

| Serveur | Port par défaut | Install macOS | Metal GPU | Facilité (1-5) | Statut |
|---|---|---|---|---|---|
| **Ollama** | 11434 | `brew install ollama` | Oui | 5 | **Supporté** — via provider custom |
| **LM Studio** | 1234 | .dmg GUI | Metal + MLX | 5 | **Supporté** — via provider custom |
| **llama-server** | 8080 | `brew install llama.cpp` | Oui | 3 | **Supporté** — via provider custom |
| **Jan.ai** | 1337 | .dmg GUI | Oui | 4 | **Supporté** — via provider custom |
| **GPT4All** | 4891 | .dmg GUI | Oui | 4 | **Supporté** — via provider custom |
| **llamafile** | 8080 | Fichier unique | Oui | 5 | **Supporté** — via provider custom |
| **LocalAI** | 8080 | Docker | Docker | 2 | **Supporté** — via provider custom |
| **KoboldCpp** | 5001 | Exécutable | Oui | 3 | **Supporté** — via provider custom |

### Natif intégré (llama-cpp-2, dans l'app)

Modèles GGUF téléchargeables depuis le Model Manager, exécutés en local via llama-cpp-2 + Metal GPU.

| Modèle | ID | Taille | Params | RAM | FR/EN | Recommandé | Statut |
|---|---|---|---|---|---|---|---|
| **Qwen3 1.7B** | `llama:qwen3-1.7b` | 1.28 GB | 1.7B | 1.5 GB | Oui | **Recommandé** | **Intégré** |
| Qwen3 4B | `llama:qwen3-4b` | 2.5 GB | 4B | 3 GB | Oui | | **Intégré** |
| Qwen3 0.6B | `llama:qwen3-0.6b` | ~400 MB | 0.6B | 600 MB | Oui | | **Intégré** |
| Gemma 3 1B | `llama:gemma3-1b` | 806 MB | 1B | 1 GB | Oui | | **Intégré** |
| Gemma 3 4B | `llama:gemma3-4b` | ~2.5 GB | 4B | 3 GB | Oui | | **Intégré** |
| SmolLM2 1.7B | `llama:smollm2-1.7b` | 1.06 GB | 1.7B | 1.3 GB | EN seul | | **Intégré** |
| SmolLM3 3B | `llama:smollm3-3b` | ~1.8 GB | 3B | 2 GB | Partiel | | **Intégré** |
| Llama 3.2 1B | `llama:llama3.2-1b` | ~700 MB | 1B | 1 GB | Partiel | | **Intégré** |
| Llama 3.2 3B | `llama:llama3.2-3b` | ~1.8 GB | 3B | 2 GB | Partiel | | **Intégré** |
| Ministral 3B | `llama:ministral3-3b` | ~1.8 GB | 3B | 2 GB | Oui | | **Intégré** |
| Phi-4 Mini | `llama:phi4-mini` | 2.49 GB | 3.8B | 3 GB | EN seul | | **Intégré** |

### Ponctuation (natif intégré)

| Modèle | ID | Params | Taille | RAM | Runtime | Langues | Vitesse | Statut |
|---|---|---|---|---|---|---|---|---|
| **Fullstop Large INT8** | `bert-punctuation:fullstop-multilang-large` | 560M | 562 MB | 600 MB | ort (CoreML) | FR, EN, DE, IT | ~100ms | **Intégré** |
| Fullstop Base FP32 | `bert-punctuation:fullstop-multilingual-base` | 280M | 1.1 GB | 560 MB | Candle (Metal) | FR, EN, DE, IT, NL | ~80ms | **Intégré** |
| PCS 47 Languages | `pcs-punctuation:47lang` | 230M | 233 MB | 300 MB | ort (CoreML) | 47 langues | ~50ms | **Intégré** |

### Modèles de ponctuation candidats

Alternatives ou compléments aux modèles intégrés.

| Modèle | Architecture | Taille | Langues | ONNX | Intérêt | Statut |
|---|---|---|---|---|---|---|
| **sherpa-onnx-online-punct-en** | CNN-BiLSTM | **7.1 MB** (int8) | EN | Oui | Ultra-léger, 1/40e la taille de BERT, 2.5x plus rapide | Écarté — EN seul, intérêt limité vs PCS 47lang |

### Modèles de correction spécialisés (alternative au LLM)

Approche pipeline : chaîner des modèles légers spécialisés au lieu d'un LLM généraliste.

| Modèle | Architecture | Params | Langues | Tâche | Intérêt | Statut |
|---|---|---|---|---|---|---|
| **FlanEC** (morenolq/flanec-large-cd) | Flan-T5 Base/Large | 250M | EN | Post-ASR error correction | Seul modèle conçu spécifiquement pour corriger les erreurs ASR | **Intégré** (`correction:flanec-large`) |
| **fdemelo/t5-base-spell-correction-fr** | T5-Base | 220M | **FR** | Correction orthographe + ponctuation | Entraîné sur corpus FR, licence MIT | **Intégré** (`correction:t5-spell-fr`) |
| **Unbabel/gec-t5_small** | T5-Small | 60M | Multilingue | Grammar error correction | Le plus petit T5 GEC, multilingue | **Intégré** (`correction:gec-t5-small`) |
| **pszemraj/flan-t5-large-grammar-synthesis** | Flan-T5-Large | 783M | EN | Grammar correction | ONNX + **GGUF** disponibles, conçu pour ASR | **Intégré** (`correction:flan-t5-grammar`) |
| **sdadas/byt5-text-correction** | ByT5 | 300M | 102+ langues | Correction character-level | Robuste aux erreurs ASR (pas de tokenizer) | Non intégré — architecture ByT5 différente, nécessiterait un runtime dédié |
| **Harper** (crate Rust) | Rule-based | N/A | EN | Grammar checking | <10ms, pure Rust, offline | Non intégré — EN seul, recherche en cours pour approche multilingue |

**Approche pipeline recommandée** (alternative au LLM, <50ms total) :
1. Regex filler words ("euh", "uh", "um", "hein", "you know") — ~0ms
2. Token classification ponctuation (7-200 MB ONNX) — ~10-30ms
3. Grammar légère : Harper (EN) / règles regex (FR) — ~5ms

---

## Audio — VAD, Denoising & Prétraitement

Voir `docs/AUDIO-PIPELINE.md` pour l'architecture complète du pipeline.

### Architecture du pipeline audio

**Pipeline actuel** :
```
Micro (cpal) → WAV 16 kHz → VAD (Silero v5) → Trim silence → ASR
```

**Pipeline hybride proposé** (denoising ciblé) :
```
Original 16 kHz WAV ──┬──► [Denoise copie] ──► VAD (boundaries précises)
                      │                              │
                      │                    trim start/end
                      │                              │
                      └──► ASR sur ORIGINAL (trimmed) ◄──┘

                      └──► [Copie dénoisée pour playback historique]
```

L'idée : le dénoisé sert à améliorer la détection VAD et le confort d'écoute, mais l'ASR reçoit toujours l'audio original (non dégradé par les artefacts spectraux du denoising).

### VAD — Comparaison détaillée

| Modèle | Architecture | Taille | Mémoire | RTF | Précision | Licence | Statut |
|---|---|---|---|---|---|---|---|
| **Silero VAD v5** | LSTM ONNX | 2.3 MB | ~5 MB | 0.006 | ROC-AUC : AliMeeting 0.96, AISHELL-4 0.94 | MIT | **✓ Intégré** (`vad.rs`) |
| **Silero VAD v6.2** | LSTM ONNX | 2 MB | ~5 MB | 0.006 | +16% vs v5 sur bruit réel, child/muted voice | MIT | **Upgrade candidat** — drop-in |
| **Earshot** | WebRTC NN (Rust pur) | **75 KB** | 8 KiB | **0.0007** | Non publié (base WebRTC NN) | MIT/Apache-2.0 | **À évaluer** — pyke.io (équipe ort), v1.0 |
| **TEN VAD** | ONNX | 2.2 MB | ~5 MB | 0.016 | Non publié, claims meilleures transitions | Apache 2.0 + **non-compete** | Écarté — clause non-compete |
| Picovoice Cobra v2.1 | Propriétaire | N/A | N/A | 0.005 | **98.9% TPR** @ 5% FPR | Propriétaire $899/mois | Écarté — coût |
| pyannote 3.0 | Transformer ONNX | 6 MB | ~20 MB | >0.05 | Excellent (diarisation) | MIT | Écarté — overkill (7 classes, chunks 10s) |
| NVIDIA MarbleNet v2 | CNN | ~400 KB | ~2 MB | ~0.002 | Bon | NVIDIA OML (restrictive) | Écarté — licence |
| WebRTC VAD (GMM) | GMM classique | 158 KB | <100 KB | ~0 | 50% TPR @ 5% FPR | BSD | Écarté — précision insuffisante |
| nnnoiseless VAD | RNNoise GRU | 85 KB | <1 MB | ~0 | ~70% | BSD-3 | Écarté — sous-produit du denoiser |

### VAD — Écosystème Rust

| Crate | Version | Modèle | Deps | Compatible ort 2.0.0-rc.11 | Notes |
|---|---|---|---|---|---|
| **`earshot`** | 1.0.0 | WebRTC NN | aucune | N/A (pas d'ort) | **75 KB, no_std, même équipe que ort** |
| `voice_activity_detector` | 0.2.1 | Silero v5 | ort rc.10 | ✗ conflit ndarray | Licence custom |
| `silero-vad-rust` | — | Silero v4/v5 | ort 1.22.x | ✗ | ORT version incompatible |
| `ten-vad-rs` | 0.1.5 | TEN VAD | ort | Potentiel | Clause non-compete |
| `webrtc-vad` | 0.4.0 | GMM | C FFI | N/A | Abandonné (2019) |
| **Direct `ort`** | 2.0.0-rc.11 | Silero | ort | ✓ | **Approche actuelle** (pas de crate VAD — conflits ndarray) |

### VAD — Prochaines étapes

1. **Immédiat** : Upgrade Silero v5 → v6.2 — swap ONNX, même API, CoreML pré-converti disponible.
2. **Court terme** : Évaluer `earshot` — 30x plus petit, 20x plus rapide, zéro dépendance.
3. **Optionnel** : Double-VAD (earshot pré-filtre rapide + Silero confirme si ambigu).

### Analyse nuancée du denoising

Le paper **"When De-noising Hurts"** (arXiv:2512.17562, déc 2025) est souvent cité pour rejeter catégoriquement le denoising. Voici une lecture plus nuancée :

**Ce que dit le paper** : le denoising dégrade l'ASR dans 40/40 configurations testées (+1% à +47% WER). Les artefacts spectraux (smearing, discontinuités) sont plus nuisibles que le bruit original pour Whisper (entraîné sur 680K heures d'audio bruité).

**Ce que le paper ne teste PAS** :
- Denoising pour **améliorer la VAD** (pré-traitement des boundaries, pas envoi direct à ASR)
- Denoising pour le **playback** (qualité d'écoute dans l'historique)
- **Canary-180M / Parakeet-TDT / Qwen3-ASR** (modèles récents hors étude)
- L'approche **hybride** : dénoisé pour VAD, original pour ASR

**Quand le denoising aide** : VAD boundaries en environnement bruité, playback historique, SNR extrême (<5 dB).

**Quand le denoising nuit** : envoi direct à ASR stock (cas testé par le paper).

**Conclusion** : pas rejeté catégoriquement, mais jamais en traitement direct pré-ASR. Utiliser en pipeline hybride ciblé.

### Denoising — Comparaison détaillée

| Modèle | Params | Taille | PESQ | STOI | DNSMOS | Sample Rate | Streaming | Licence | Statut |
|---|---|---|---|---|---|---|---|---|---|
| **nnnoiseless** | 60K | 85 KB | **3.88** | 0.92 | — | 48 kHz | Oui | BSD-3 | **Candidat #1** — Rust pur, inclut VAD bonus |
| **GTCRN** | 48K | <100 KB | 2.87 | 0.940 | 3.44 | **16 kHz** | Oui | MIT | **Candidat #2** — ultra-léger, ort natif |
| **UL-UNAS** | 169K | ~500 KB | 3.09 | — | — | **16 kHz** | Oui | MIT | **Candidat #3** — évolution GTCRN |
| **DTLN** | <1M | <4 MB | 3.04 | — | — | **16 kHz** | Oui (stateful) | MIT | Alternative — plus lourd |
| **DeepFilterNet3** | 2.13M | ~8 MB | 3.17 | 0.944 | — | 48 kHz | Oui | MIT/Apache | Meilleure qualité — mais 48 kHz, tract, stale |
| NSNet2 | ~6M | ~20 MB | 2.94 | — | — | 16 kHz | Problématique | MIT | Écarté — GRU state streaming issue |
| FRCRN | 10.3M | N/A | — | — | — | — | Non | — | Écarté — trop lourd, pas d'ONNX |
| Demucs | ~135 MB | 135 MB | — | — | — | 44.1 kHz | Non | CC-BY-NC | Écarté — licence, taille |

### Denoising — Écosystème Rust

| Option | Crate/Méthode | Runtime | 16 kHz natif | Effort intégration |
|---|---|---|---|---|
| **nnnoiseless** | `nnnoiseless` 0.5.2 | Rust pur | Non (48 kHz, `rubato` requis) | Faible |
| **GTCRN via ort** | ONNX direct | `ort` + CoreML | **Oui** | Moyen (STFT/ISTFT manuelle) |
| **UL-UNAS via ort** | ONNX direct | `ort` + CoreML | **Oui** | Moyen (STFT/ISTFT manuelle) |
| **DTLN via ort** | ONNX direct | `ort` | **Oui** | Moyen (stateful) |
| **DeepFilterNet3** | `deep_filter` (git) | `tract` | Non (48 kHz) | Élevé (tract + 3 models) |

### Denoising — Pipeline hybride recommandé

Recommandation denoiser selon priorité :
- **16 kHz natif préféré** : GTCRN (<100 KB, ort) ou UL-UNAS (~500 KB, meilleur PESQ)
- **Qualité max** : nnnoiseless (PESQ 3.88, Rust pur, mais 48 kHz → `rubato` resampling)
- **Meilleur équilibre** : UL-UNAS (16 kHz, PESQ 3.09, MIT, ort, CoreML)

### Silence Trimming — Techniques classiques

| Technique | Coût CPU | Précision (propre) | Précision (bruité) | Notes |
|---|---|---|---|---|
| Seuil énergie RMS | ~0 | Bonne | Mauvaise | Inutilisable en bruit ambiant |
| Zero-Crossing Rate | ~0 | Médiocre | Très mauvaise | Utile uniquement combiné |
| Double seuil adaptatif | ~0 | Modérée | Médiocre | Mieux que RMS seul |
| Entropie spectrale | Faible (FFT) | Bonne | Modérée | Transitions signal/bruit |
| **VAD neuronale (Silero)** | 189µs/chunk | **Excellente** | **Excellente** | **✓ Approche actuelle** |
| **VAD neuronale (Earshot)** | <100µs/chunk | À évaluer | À évaluer | Candidat benchmark |

La VAD neuronale est sans égale. Les techniques classiques ne sont pertinentes que comme pré-filtre ultra-rapide.

### Mises à niveau recommandées (roadmap)

| Priorité | Action | Effort | Impact |
|---|---|---|---|
| **1 — Immédiat** | Upgrade Silero v5 → v6.2 | Très faible (swap .onnx) | -16% erreurs VAD, meilleure gestion voix difficiles |
| **1 — Immédiat** | Corriger documentation v5/v6 | Trivial | Cohérence code/docs |
| **2 — Court terme** | Évaluer Earshot sur audio réel | Faible (add crate, A/B test) | Potentiel : -95% taille VAD, +20x vitesse |
| **3 — Moyen terme** | Pipeline hybride denoise-for-VAD | Modéré | Meilleur trimming en environnement bruité |
| **4 — Optionnel** | Denoising pour playback historique | Modéré | UX : audio propre en relecture |

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
