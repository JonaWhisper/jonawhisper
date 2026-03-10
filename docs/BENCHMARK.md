# Benchmark ASR & LLM — Mars 2026

Référence complète des options ASR (speech-to-text) et LLM (text cleanup) disponibles pour JonaWhisper. Couvre le cloud, le self-hosted, et le natif intégré.

---

## ASR — Speech-to-Text

> **Cloud ASR** (providers OpenAI-compatible, APIs propriétaires, self-hosted) : voir [`CLOUD-PROVIDERS.md`](CLOUD-PROVIDERS.md)

### Natif intégré — Whisper (whisper-rs, Metal GPU)

Modèles GGML téléchargeables depuis le Model Manager, exécutés en local via whisper-rs + Metal GPU.

| Modèle | ID | Taille | RAM | WER | RTF | Recommandé | Statut |
|---|---|---|---|---|---|---|---|
| Large V3 | `whisper:large-v3` | 3.1 GB | 4 GB | 1.8% | 0.50 | | **Intégré** |
| Large V2 | `whisper:large-v2` | 3.09 GB | 4 GB | 1.9% | 0.50 | | **Intégré** |
| **Large V3 Turbo** | `whisper:large-v3-turbo` | 1.6 GB | 2.5 GB | 2.1% | 0.25 | **Recommandé** | **Intégré** |
| Large V3 Turbo Q8 | `whisper:large-v3-turbo-q8` | 874 MB | 1.3 GB | 2.1% | 0.20 | | **Intégré** |
| Large V3 Turbo Q5 | `whisper:large-v3-turbo-q5` | 574 MB | 900 MB | 2.3% | 0.15 | | **Intégré** |
| Large V3 French | `whisper:large-v3-french-distil` | 538 MB | 900 MB | 1.5% | 0.20 | | **Intégré** |
| Medium | `whisper:medium` | 1.5 GB | 2 GB | 2.7% | 0.35 | | **Intégré** |
| Medium Q5 | `whisper:medium-q5` | 539 MB | 900 MB | 2.8% | 0.20 | | **Intégré** |
| Small | `whisper:small` | 466 MB | 750 MB | 3.4% | 0.15 | | **Intégré** |
| Small Q5 | `whisper:small-q5` | 190 MB | 400 MB | 3.6% | 0.10 | | **Intégré** |
| Base | `whisper:base` | 142 MB | 300 MB | 5.0% | 0.08 | | **Intégré** |
| Tiny | `whisper:tiny` | 75 MB | 200 MB | 7.6% | 0.05 | | **Intégré** |

#### Distil-Whisper

| Modèle | Params | Format | Taille | Langues | WER | Vitesse | Statut |
|---|---|---|---|---|---|---|---|
| **Distil-Whisper Large v3.5** | 756M | ONNX | ~1.5 GB | **EN seul** | 7.08% short / 11.39% long | 1.5x Turbo | Écarté — EN seul |
| Distil-Whisper Large v3 | 756M | ONNX | ~1.5 GB | EN seul | ~7% | 6.3x Whisper | Écarté — EN seul |

**Note** : les Distil-Whisper sont EN seul → pas adaptés à notre cas multilingue. Mentionnés pour complétude. Turbo Q8 offre un meilleur compromis taille/qualité/langues.

### Natif intégré — Modèles ASR non-Whisper

#### Modèles intégrés

| Modèle | Params | FR | Format | Taille | RAM | Architecture | Streaming | Runtime | Statut |
|---|---|---|---|---|---|---|---|---|---|
| **Whisper Large V3 French** | 1.5B | Natif FR | GGML | 538 MB | 900 MB | Encoder-decoder attention | Non | whisper-rs (Metal) | **Intégré** (`whisper:large-v3-french-distil`) |
| **Canary-180M-Flash** | 182M | 4 langues | ONNX int8 | 213 MB | 300 MB | FastConformer enc-dec | Non | ort (CoreML) | **Intégré** (`canary:180m-flash-int8`) |
| **Parakeet-TDT 0.6B v3** | 600M | 25 langues | ONNX int8 | 703 MB | 750 MB | FastConformer + TDT transducer | Non | ort (CoreML) | **Intégré** (`parakeet:tdt-0.6b-v3-int8`) |
| **Qwen3-ASR 0.6B** | 600M | 30 langues | Safetensors | 1.88 GB | 2 GB | Qwen encoder-decoder | Oui (crate) | qwen-asr (Accelerate/AMX) | **Intégré** (`qwen-asr:0.6b`) |
| **Voxtral Realtime 4B** | 4.4B | 13 langues | Safetensors BF16 | 8.9 GB | ~10 GB | Mimi encoder + LLM decoder | Oui (voxtral.c) | voxtral.c vendoré (Metal) | **Intégré** (`voxtral:mini-4b-realtime`) |

#### Voxtral — Benchmarks (WER moyen FLEURS + MCV + MLS)

D'après le benchmark Mistral (chart officiel), WER moyen sur 6 langues :

| Langue | Voxtral Small 24B | Voxtral Mini 3B (Transcribe) | Voxtral Mini (local) | Whisper large-v3 |
|---|---|---|---|---|
| Spanish | ~3.2% | ~3.6% | ~4.5% | ~4.2% |
| German | ~4.2% | ~4.7% | ~5.9% | ~5.8% |
| French | ~4.8% | ~5.3% | ~5.3% | ~5.6% |
| Italian | ~5.1% | ~5.4% | ~6.6% | ~6.3% |
| Portuguese | ~5.4% | ~5.1% | ~6.4% | ~6.3% |
| Dutch | ~6.1% | ~6.4% | ~7.4% | ~6.9% |

**Note** : le Realtime 4B (intégré) n'apparaît pas dans ce benchmark — c'est un modèle streaming antérieur aux Mini 3B / Small 24B. Les Mini 3B et Small 24B utilisent une architecture différente (Pixtral multimodal) non compatible avec voxtral.c. Seul le Realtime 4B fonctionne avec le moteur voxtral.c actuel.

#### Candidats intégrables

Modèles avec un chemin d'intégration réaliste (ONNX disponible, safetensors, crate Rust existant) :

| Modèle | Params | FR | Format | Taille | Runtime Rust | Langues | WER | Intérêt | Statut |
|---|---|---|---|---|---|---|---|---|---|
| **Qwen3-ASR 1.7B** | 1.7B | Oui | Safetensors | 4.7 GB | `qwen-asr` (Rust pur, AMX) | 30 + dialectes | 1.63% LS | Meilleur WER open-source, streaming, FR natif | **Candidat #1** |
| **Canary-1B v2** | 978M | Oui | ONNX (community) | ~2 GB | ort (CoreML) | 25 EU | 7.27-8.85% | 25 langues EU, traduction bidirectionnelle | **Candidat #2** |
| **SenseVoice Small** | 234M | 50+ (focus zh/en) | ONNX int8 | 228 MB | `sensevoice-rs` (Candle) | 50+ | Excellent zh/en | Ultra-rapide (15x Whisper), ONNX + Rust crate | **Candidat #3** |
| **Moonshine v2 Medium** | 250M | EN seul | ONNX (.ort) | ~500 MB | ort | EN | ~6.65% | 100x Whisper Large, streaming natif | Écarté — EN seul |
| **Moonshine Tiny** | 27M | EN seul | ONNX (.ort) | 108 MB | ort | EN | ~12.7% | Ultra-compact, <108 MB | Écarté — EN seul |
| ~~Voxtral Mini 4B~~ | 4B | Oui | Safetensors BF16 | 8.87 GB | voxtral.c (C, Metal) | 13 langues | — | **✓ Intégré** via voxtral.c vendoré | **Intégré** (`voxtral:mini-4b-realtime`) |
| **Canary-1B-Flash** | ~1B | 4 langues | .nemo | ~2 GB | Conversion ONNX nécessaire | FR/EN/DE/ES | — | >1000 RTFx, streaming | Non intégré — conversion .nemo→ONNX non triviale |
| **OWSM-CTC v4 1B** | ~1B | Multi | PyTorch | ~2 GB | Conversion ONNX nécessaire | Multi | — | Encoder-only CTC, ultra-rapide | Non intégré — ESPnet/PyTorch, pas d'ONNX |
| **IBM Granite Speech 3.3 8B** | 8B | Oui | Safetensors | ~16 GB | Candle (théorique) | Multi | 5.85% | Top leaderboard, Apache 2.0 | Écarté — 16 GB, LLM backbone trop lourd |
| **Parakeet-TDT 1.1B** | 1.1B | EN seul | .nemo | ~2.2 GB | Pas d'ONNX officiel | EN | 1.39% LS | Meilleur WER EN, RTFx >2000 | Écarté — EN seul, pas d'ONNX |

#### Modèles recherche / non-intégrables

Modèles importants dans l'écosystème mais sans chemin d'intégration pratique :

| Modèle | Params | FR | Raison d'exclusion |
|---|---|---|---|
| **Meta MMS 1B** | 1B | Oui (1162 langues) | CC-BY-NC (non-commercial), pas d'ONNX, pas de crate Rust |
| **Meta SeamlessM4T v2** | 3B | Oui (96 langues) | Trop lourd (3B), PyTorch only, pas d'ONNX |
| **Google USM** | 2B | Oui (100+ langues) | Propriétaire Google, pas de poids publics |
| **Apple Foundation Speech** | ~3B | Multi | Propriétaire Apple, intégré iOS/macOS uniquement |
| **NVIDIA Canary-Qwen 2.5B** | 2.5B | Oui (25 langues) | Top leaderboard (5.63% WER) mais 2.5B = trop lourd natif |
| **Voxtral Small 24B** | 24B | Oui | Architecture Pixtral (pas voxtral.c), trop lourd pour local |
| **Voxtral Mini 3B** | 3B | Oui | Architecture Pixtral (pas voxtral.c), pas de runtime Rust compatible |
| **GigaAM v3** | 240M | **Russe seul** | Focus russe uniquement, pas de FR |
| **GLM-ASR-Nano** | 1.5B | **ZH/EN/Cantonais** | Focus chinois, pas de FR |
| **FunASR Paraformer** | Var. | ZH/EN | Focus chinois, complexe à intégrer |
| Wav2Vec 2.0 / HuBERT / WavLM | 300M-1B | Via fine-tune | Ancienne génération, nécessite fine-tuning par langue |

### Open ASR Leaderboard — Résumé

Résumé des top modèles du leaderboard HuggingFace (60+ modèles, 18 organisations, 11 datasets) :

| Rang | Modèle | WER moyen | RTFx | Params | Intégrable | Notes |
|---|---|---|---|---|---|---|
| 1 | NVIDIA Canary-Qwen 2.5B | 5.63% | 418 | 2.5B | Non (trop lourd) | Top accuracy |
| 2 | IBM Granite Speech 3.3 8B | 5.85% | — | 8B | Non (trop lourd) | Apache 2.0 |
| 3 | Deepgram Nova-3 (cloud) | 5.26% | — | Propriétaire | Cloud uniquement | Meilleur WER cloud |
| — | NVIDIA Parakeet CTC 1.1B | 6.68% | **2794** | 1.1B | Non (EN seul) | SOTA vitesse |
| — | OpenAI Whisper Large V3 | 6.43% | 69 | 1.55B | **Oui** (intégré) | Référence multilingue |
| — | Moonshine Medium v2 | 6.65% | ~1200 | 250M | Non (EN seul) | 100x Whisper |
| — | Qwen3-ASR 1.7B | ~4.5% estimé | — | 1.7B | **Oui** (candidat) | 1.63% LS clean |

**Note** : les meilleurs modèles du leaderboard (Canary-Qwen, Granite) sont trop lourds pour du natif. Les modèles intégrables offrent un bon compromis qualité/taille.

### Écosystème Rust ASR

| Crate | Modèles supportés | Runtime | Licence | Notes |
|---|---|---|---|---|
| `whisper-rs` 0.15+ | Whisper (GGML) | whisper.cpp (Metal) | MIT | Production, notre runtime principal |
| `qwen-asr` | Qwen3-ASR 0.6B/1.7B | Rust pur (AMX/NEON) | Apache 2.0 | Pure Rust, zero deps, streaming |
| `ort` 2.0.0-rc.11 | Canary, Parakeet, tout ONNX | ONNX Runtime (CoreML) | MIT/Apache | Notre runtime ONNX |
| `sensevoice-rs` | SenseVoice Small | Candle (Metal) | MIT | 50+ langues, auto-download hf-hub |
| **voxtral.c** (vendoré) | Voxtral Realtime 4B | C pur + Metal GPU | MIT | **✓ Intégré**, 13 langues, ~8.9 GB BF16 |
| `parakeet-rs` | Parakeet (ONNX) | ort | MIT | Streaming + punctuation |
| `sherpa-rs` (sherpa-onnx) | Zipformer, Paraformer, etc. | ort (C bindings) | Apache 2.0 | Zoo de modèles multilingues |
| `april_asr` | Modèles April | Natif | MIT | Offline streaming minimal |
| Burn + voxtral-mini-realtime-rs | Voxtral 4B | Burn (Vulkan/Metal) | Apache 2.0 | Communauté, expérimental (non utilisé — on utilise voxtral.c directement) |

### Roadmap ASR

| Priorité | Action | Effort | Impact |
|---|---|---|---|
| **1 — Immédiat** | Intégrer Qwen3-ASR 1.7B (via `qwen-asr` existant) | Très faible | Meilleur WER, 30 langues, streaming |
| **2 — Court terme** | Évaluer Canary-1B v2 (ONNX community) | Modéré | 25 langues EU, traduction intégrée |
| **3 — Court terme** | Évaluer SenseVoice Small (`sensevoice-rs`) | Faible | Ultra-rapide, 228 MB, crate Rust |
| **4 — Optionnel** | Moonshine Tiny pour mode ultra-léger EN | Faible | 108 MB, streaming, EN seul |
| **5 — Optionnel** | Adapter cloud providers (Deepgram, AssemblyAI streaming FR) | Modéré | Alternatives cloud premium |

---

## LLM — Text Cleanup

> **Cloud LLM** (providers OpenAI-compatible, APIs propriétaires, self-hosted) : voir [`CLOUD-PROVIDERS.md`](CLOUD-PROVIDERS.md)

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
Pipeline texte actuel (8 étapes) :
  ASR brut → [1. Hallucinations] → [2. Dictée] → [3. Disfluences]
           → [4. Ponctuation] → [5. Spell-check] → [6. Correction/LLM] → [7. Finalize] → [8. ITN] → Paste
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
| **GEC T5 Small** | `correction:gec-t5-small` | 60M | 96 MB | Multilingue (11 langues) | ~200ms | Repeat penalty 1.5, n-gram block | **Intégré** — **Recommandé** |
| T5 Spell FR | `correction:t5-spell-fr` | 220M | 276 MB | FR | ~500ms | Repeat penalty 1.5, n-gram block | **Intégré** |
| FlanEC Base | `correction:flanec-base` | 250M | 276 MB | EN | ~500ms | Repeat penalty 1.5, n-gram block | **Intégré** |
| FlanEC Large | `correction:flanec-large` | 800M | 821 MB | EN | ~1s | Repeat penalty 1.5, n-gram block | **Intégré** |

Tous utilisent le runtime **ort** (ONNX Runtime + CoreML) avec décodage autorégressif, repeat penalty 1.5, n-gram blocking (taille 4), détection de boucle live. Sortie sanitisée : vide → garder l'original, >3x longueur input → garder l'original (protection anti-hallucination).

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

**Approche choisie** : regex simple (~0ms, fiabilité >95% sur fillers isolés).

| Approche | Latence | Précision | Complexité | Statut |
|---|---|---|---|---|
| **Regex fillers** | ~0ms | >95% (fillers isolés) | Triviale | ✅ **Implémenté** (`post_processor.rs`) |
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
| **Regex rules FR/EN** | FR, EN | ~80% (cas courants) | Native | ✅ **Implémenté** (`cleanup/itn.rs`) — nombres, ordinaux, %, heures, devises, unités |
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
| 3 | Suppression disfluences | Regex fillers FR/EN | ~0ms | `post_processor.rs` | ✅ Implémenté |
| 4 | Ponctuation + Capitalisation | BERT/PCS token classification | ~50-100ms | `crates/jona-engine-bert/`, `crates/jona-engine-pcs/` | ✅ Implémenté |
| 5 | Spell-check | SymSpell (downloadable dicts FR/EN) | ~5-10ms | `cleanup/symspell_correct.rs` | ✅ Implémenté |
| 6 | Correction gram/ortho | T5 encoder-decoder autorégressif (ONNX) | ~200ms-1s | `crates/jona-engine-correction/` | ✅ Implémenté |
| 7 | Finalize | Espacement ponctuation + capitalisation initiale | ~0ms | `post_processor.rs` | ✅ Implémenté |
| 8 | ITN | Regex nombres/dates/heures FR/EN | ~0ms | `cleanup/itn.rs` | ✅ Implémenté |

**Pipeline séquentiel** : ponctuation et correction sont chaînés séquentiellement (ponctuation → spell-check → correction/LLM). Chacun a son propre paramètre indépendant. Voir `docs/TEXT-PIPELINE.md` pour le détail.

### Roadmap post-traitement texte

| Priorité | Action | Effort | Impact |
|---|---|---|---|
| **1 — Moyen terme** | Évaluer GECToR (tag-based, 10x T5) | Modéré | Correction ~20ms sans hallucination |
| **2 — Optionnel** | Évaluer Harper (rule-based Rust, EN) | Faible | Correction instantanée, complémentaire |
| **3 — Optionnel** | ITN WFST (NeMo/Sparrowhawk) pour couverture complète | Élevé | >95% ITN multi-langues |

---

## Audio — VAD, Denoising & Prétraitement

Voir `docs/AUDIO-PIPELINE.md` pour l'architecture complète du pipeline.

### Architecture du pipeline audio

**Pipeline actuel** :
```
Micro (cpal) → WAV 16 kHz → VAD (Silero v6.2) → Trim silence → ASR
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
| **Silero VAD v6.2** | LSTM ONNX | 2.3 MB | ~5 MB | 0.006 | ROC-AUC : AliMeeting 0.96, AISHELL-4 0.94. +16% vs v5 sur bruit réel, voix enfants/étouffées | MIT | **✓ Intégré** (`vad.rs`) |
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
| `voice_activity_detector` | 0.2.1 | Silero v6 | ort rc.10 | ✗ conflit ndarray | Licence custom |
| `silero-vad-rust` | — | Silero v4/v5 | ort 1.22.x | ✗ | ORT version incompatible |
| `ten-vad-rs` | 0.1.5 | TEN VAD | ort | Potentiel | Clause non-compete |
| `webrtc-vad` | 0.4.0 | GMM | C FFI | N/A | Abandonné (2019) |
| **Direct `ort`** | 2.0.0-rc.11 | Silero v6.2 | ort | ✓ | **Approche actuelle** (pas de crate VAD — conflits ndarray) |

### VAD — Prochaines étapes

1. ~~**Immédiat** : Upgrade Silero v5 → v6.2~~ — **✓ Fait** (modèle ONNX déjà v6.2, même API).
2. **Court terme** : Évaluer `earshot` — 30x plus petit, 20x plus rapide, zéro dépendance.
3. **Optionnel** : Double-VAD (earshot pré-filtre rapide + Silero confirme si ambigu).

### Analyse nuancée du denoising

Le paper **"When De-noising Hurts"** (arXiv:2512.17562, déc 2025) est souvent cité pour rejeter catégoriquement le denoising. Voici une lecture plus nuancée :

**Ce que dit le paper** : le denoising dégrade l'ASR dans 40/40 configurations testées (+1% à +47% WER). Les artefacts spectraux (smearing, discontinuités) sont plus nuisibles que le bruit original pour Whisper (entraîné sur 680K heures d'audio bruité).

**Ce que le paper ne teste PAS** :
- Denoising pour **améliorer la VAD** (pré-traitement des boundaries, pas envoi direct à ASR)
- Denoising pour le **playback** (qualité d'écoute dans l'historique)
- **Canary-180M / Parakeet-TDT / Qwen3-ASR / Voxtral** (modèles récents hors étude)
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
| ~~**1 — Immédiat**~~ | ~~Upgrade Silero → v6.2~~ | ~~Très faible~~ | **✓ Fait** — modèle ONNX déjà v6.2, docs mises à jour |
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
