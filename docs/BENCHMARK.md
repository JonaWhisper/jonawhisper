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

## Post-traitement texte — Ponctuation, Correction & Formatage

Voir `docs/TEXT-PIPELINE.md` pour l'architecture complète du pipeline texte post-ASR.

```
Pipeline texte actuel (7 étapes) :
  ASR brut → [1. Filtre hallucinations] → [2. Commandes dictée] → [3. —]
           → [4. Ponctuation OU 5. Correction OU LLM] → [7. Finalize] → Paste
```

### Ponctuation & Capitalisation — Modèles intégrés

| Modèle | ID | Params | Taille | RAM | Runtime | Langues | Capitalisation | Vitesse | Statut |
|---|---|---|---|---|---|---|---|---|---|
| **Fullstop Large INT8** | `bert-punctuation:fullstop-multilang-large` | 560M | 562 MB | 600 MB | ort (CoreML) | FR, EN, DE, IT | Non | ~100ms | **Intégré** |
| Fullstop Base FP32 | `bert-punctuation:fullstop-multilingual-base` | 280M | 1.1 GB | 560 MB | Candle (Metal) | FR, EN, DE, IT, NL | Non | ~80ms | **Intégré** |
| **PCS 47 Languages** | `pcs-punctuation:47lang` | 230M | 233 MB | 300 MB | ort (CoreML) | 47 langues | **Oui** (4 heads) | ~50ms | **Intégré** |

**Note** : seul PCS restaure la casse (capitalisation). Les modèles BERT fullstop ne prédisent que la ponctuation (`.` `,` `?` `-` `:`). La capitalisation post-BERT repose uniquement sur le `finalize()` de `post_processor.rs` (début de phrase + après `.?!`).

### Ponctuation — Modèles candidats

| Modèle | Architecture | Taille | Langues | Capitalisation | Intérêt | Statut |
|---|---|---|---|---|---|---|
| **EdgePunct** (sherpa-onnx) | CNN-BiLSTM INT8 | **7.1 MB** | EN | **Oui** (inclut casing) | Ultra-léger, 1/40e de BERT, 2.5x plus rapide | Écarté — EN seul, PCS couvre 47 langues |
| **FunASR ct-punc** | Transformer | ~300 MB | ZH, EN | Non | Alibaba, haute précision chinois | Écarté — focus chinois, pas de FR |
| **Mark My Words / Cadence** | Gemma3-1B fine-tuné | ~2 GB | Multi | Oui | 30 classes ponctuation, mars 2025 | Non intégré — 1B params trop lourd, trop récent |
| **ELECTRA-Small Punct** | ELECTRA | 13 MB | EN | Non | Ultra-léger, latence 3-4 mots | Écarté — EN seul |
| **Universal-2-TF** (AssemblyAI) | Transformer | Propriétaire | Multi | Oui + ITN | Ponctuation + truecasing + ITN combiné | Écarté — propriétaire, cloud uniquement |
| **DeepPunct** | Transformer | ~150 MB | EN | Non | Ponctuation contextuelle | Écarté — EN seul, pas maintenu |

**Conclusion** : PCS reste le meilleur choix intégré — 47 langues, capitalisation native (4 heads : pre-punct, post-punct, casing, segmentation), 233 MB, ~50ms, SentencePiece tokenizer universel.

### Correction grammaticale & orthographique — Modèles intégrés

| Modèle | ID | Params | Taille | Langues | Vitesse | Anti-hallucination | Statut |
|---|---|---|---|---|---|---|---|
| **GEC T5 Small** | `correction:gec-t5-small` | 60M | 242 MB | Multilingue (11 langues) | ~200ms | Repeat penalty 1.1, n-gram block | **Intégré** — **Recommandé** |
| T5 Spell FR | `correction:t5-spell-fr` | 220M | 892 MB | FR | ~500ms | Repeat penalty 1.1, n-gram block | **Intégré** |
| FlanEC Large | `correction:flanec-large` | 250M | 990 MB | EN | ~800ms | Repeat penalty 1.1, n-gram block | **Intégré** |
| Flan-T5 Grammar | `correction:flan-t5-grammar` | 783M | 3.1 GB | EN | ~2s | Repeat penalty 1.1, n-gram block | **Intégré** |

Tous utilisent le runtime **Candle** (Metal GPU) avec décodage autorégressif, KV cache, température 0.1. Sortie sanitisée : vide → garder l'original, >3x longueur input → garder l'original (protection anti-hallucination).

### Correction — Modèles candidats

| Modèle | Architecture | Params | Langues | Vitesse | Intérêt | Statut |
|---|---|---|---|---|---|---|
| **GECToR** (Grammarly) | BERT + 2 linear, tag-based | ~110M | EN | **~20ms** (10x T5) | Tags (KEEP/DELETE/REPLACE), pas autorégressif → pas de hallucination | **Candidat sérieux** — nécessite ONNX export |
| **ByT5** (sdadas) | ByT5 character-level | 300M | 102+ langues | ~1s | Robuste aux erreurs ASR (opère au caractère) | Non intégré — architecture ByT5 non supportée par Candle |
| **Harper** (crate Rust) | Rule-based | N/A | EN | **<10ms** | Rust pur, MIT, offline, `harper-core` crate | **Candidat complémentaire** — EN seul, léger |
| **LanguageTool** | Rules + ngram | N/A | 25+ langues | ~50ms | Le plus complet, 25+ langues | Écarté — Java runtime requis, 200 MB+ |
| **GECFramework** (ACL 2024) | Detection-Correction | Var. | Multi | Var. | Architecture detection+correction deux passes | Non intégré — Python only |
| **Gramformer** | T5/BART GEC | 220M | EN | ~500ms | Fine-tuné GEC | Écarté — doublon des T5 intégrés |

**GECToR** est le candidat le plus intéressant : approche tag-based (non-autorégressif) = aucun risque de hallucination, latence ~20ms vs ~200ms-2s pour T5. Limitation : EN seul pour le modèle Grammarly original. Nécessiterait un ONNX export du modèle PyTorch.

### Suppression des disfluences

Les fillers/disfluences sont des mots parasites émis naturellement à l'oral. Les modèles ASR les transcrivent fidèlement.

| Langue | Fillers courants |
|---|---|
| FR | euh, heu, hum, bah, ben, beh, enfin, quoi, genre, voilà, du coup, en fait, tu vois |
| EN | uh, um, hmm, like, you know, I mean, basically, actually, so, well, right |

**Approche recommandée** : regex simple (~0ms, fiabilité >95% sur fillers isolés).

| Approche | Latence | Précision | Complexité | Statut |
|---|---|---|---|---|
| **Regex fillers** | ~0ms | >95% (fillers isolés) | Triviale | ❌ Non implémenté |
| CTC Forced Alignment | ~100ms | 81.6% (disfluences complexes) | Élevée (modèle CTC + alignement) | Écarté — trop lourd |
| Smooth-LLaMa | ~500ms | ~85% | Élevée (LLM fine-tuné) | Écarté — trop lourd |
| Whisper word timestamps | ~0ms (déjà disponible) | Variable | Moyenne | Non exploré — nécessite word-level timestamps |

**Conclusion** : un regex couvrant les fillers classiques FR/EN est suffisant pour la dictée. Les disfluences complexes (faux départs, répétitions) sont rares en dictée volontaire vs conversation spontanée.

### ITN — Inverse Text Normalization

Convertit les nombres et entités textuelles en forme écrite canonique.

| Exemple entrée | Sortie attendue | Catégorie |
|---|---|---|
| vingt-trois | 23 | Nombre |
| dix pour cent | 10% | Pourcentage |
| trois heures et quart | 3h15 | Heure |
| cinq euros cinquante | 5,50 € | Devise |
| premier janvier deux mille vingt-cinq | 1er janvier 2025 | Date |
| twenty three | 23 | Number (EN) |
| ten percent | 10% | Percentage (EN) |

| Approche | Langues | Précision | Portabilité Rust | Statut |
|---|---|---|---|---|
| **Regex rules FR/EN** | FR, EN | ~80% (cas courants) | Native | **Candidat immédiat** — couvre nombres, %, heures, devises |
| **NeMo ITN** (NVIDIA) | Multi (WFST) | >95% | Difficile (WFST C++) | Non intégré — WFST complexe à porter en Rust |
| **Thutmose Tagger** | Multi (neural) | >90% | Difficile (Python, pas d'ONNX) | Non intégré — pas d'export ONNX disponible |
| **Sparrowhawk** (Google) | Multi (WFST C++) | >95% | FFI possible mais complexe | Non intégré — FFI C++ + grammaires WFST |
| LLM prompt | Multi | Variable | Via LLM existant | Non intégré — déjà couvert par le cleanup LLM si activé |

**Conclusion** : un jeu de regex FR/EN couvrant nombres cardinaux/ordinaux, pourcentages, heures et devises est le meilleur rapport effort/valeur. Les cas edge (grands nombres composés, dates complexes) peuvent être délégués au LLM si activé.

### Écosystème Rust — Crates pertinents post-traitement

| Crate | Version | Usage | Licence | Notes |
|---|---|---|---|---|
| `harper-core` | 0.x | Grammar checking rule-based | MIT | EN seul, <10ms, offline |
| `nlprule` | 0.6 | LanguageTool rules en Rust | MIT/Apache | Bindings Rust de LanguageTool, limité |
| `rust-stemmers` | 1.2 | Stemming multi-langues | MIT | Utile pour normalisation |
| `unicode-segmentation` | 1.x | Segmentation texte Unicode | MIT/Apache | Déjà utilisé indirectement |
| `whatlang` | 0.16 | Détection de langue | MIT | Alternative au resolve_language() actuel |
| `num-to-words` | 0.1 | Nombres → mots (EN) | MIT | Utile pour ITN inverse (validation) |

### Pipeline recommandé (7 étapes)

| # | Étape | Traitement | Latence | Fichier | Statut |
|---|---|---|---|---|---|
| 1 | Filtre hallucinations | Regex 30+ patterns (HALLUCINATIONS) | ~0ms | `post_processor.rs` | ✅ Implémenté |
| 2 | Commandes dictée | Substitution FR/EN ("virgule" → ",") | ~0ms | `post_processor.rs` | ✅ Implémenté |
| 3 | Suppression disfluences | Regex fillers FR/EN | ~0ms | — | ❌ Non implémenté |
| 4 | Ponctuation + Capitalisation | BERT/PCS token classification | ~50-100ms | `bert_punctuation.rs`, `candle_punctuation.rs`, `pcs_punctuation.rs` | ✅ Implémenté |
| 5 | Correction gram/ortho | T5 encoder-decoder autorégressif | ~200ms-2s | `t5_correction.rs` | ✅ Implémenté |
| 6 | ITN | Regex nombres/dates/heures | ~0ms | — | ❌ Non implémenté |
| 7 | Finalize | Espacement ponctuation + capitalisation initiale | ~0ms | `post_processor.rs` | ✅ Implémenté |

**Limitation actuelle** : les étapes 4 et 5 sont mutuellement exclusives (ponctuation OU correction OU LLM, pas chaînés). Le chaînage ponctuation → correction est une amélioration future. Voir `docs/TEXT-PIPELINE.md` pour le détail.

### Roadmap post-traitement texte

| Priorité | Action | Effort | Impact |
|---|---|---|---|
| **1 — Immédiat** | Regex disfluences FR/EN (étape 3) | Très faible | Texte plus propre, ~0ms |
| **2 — Court terme** | Regex ITN nombres/heures FR/EN (étape 6) | Faible | "vingt-trois" → "23" |
| **3 — Court terme** | Chaînage ponctuation + correction | Modéré | Pipeline complet au lieu de OU exclusif |
| **4 — Moyen terme** | Évaluer GECToR (tag-based, 10x T5) | Modéré | Correction ~20ms sans hallucination |
| **5 — Moyen terme** | Évaluer Harper (rule-based Rust, EN) | Faible | Correction instantanée, complémentaire |
| **6 — Optionnel** | ITN WFST (NeMo/Sparrowhawk) pour couverture complète | Élevé | >95% ITN multi-langues |

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
