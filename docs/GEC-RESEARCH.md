# Recherche — Correction grammaticale & spell-check

*Mars 2026 — Synth\u00e8se compl\u00e8te de la recherche pour am\u00e9liorer le pipeline de correction texte FR/EN.*

---

## Contexte

Le pipeline actuel :
- **Ponctuation** : PCS (47 langues, ONNX+CoreML) ou BERT (4 langues, ONNX+CoreML) — stable, rapide
- **Correction T5** : 4 mod\u00e8les via Candle (safetensors + Metal) — probl\u00e8mes de stabilit\u00e9 (boucles, crashes Metal)
- **LLM** : local (llama-cpp) ou cloud (OpenAI/Anthropic) — lourd mais flexible

L'objectif : trouver des alternatives plus fiables, rapides, et couvrant bien FR + EN.

---

## 1. Spell-checking pur Rust

### spellbook (recommand\u00e9)

[GitHub](https://github.com/helix-editor/spellbook) | [crates.io](https://crates.io/crates/spellbook) | [docs.rs](https://docs.rs/spellbook)

| Propri\u00e9t\u00e9 | Valeur |
|---|---|
| Version | 0.3.4 (avril 2025) |
| Maintenu par | \u00c9quipe Helix editor |
| Base | R\u00e9\u00e9criture Rust de Nuspell (successeur C++ de Hunspell) |
| D\u00e9pendances | `hashbrown` uniquement, `no_std` |
| Suggestions | Fonctionnelles (depuis v0.2.0), m\u00eame algo que Nuspell |
| Statut | Alpha (API peut changer), mais tests Hunspell corpus passent |

**API** :
```rust
let aff = std::fs::read_to_string("fr.aff")?;
let dic = std::fs::read_to_string("fr.dic")?;
let dict = spellbook::Dictionary::new(&aff, &dic)?;

dict.check("bonjour");  // true
dict.check("bonojur");  // false
// Suggestions disponibles via Suggester
```

**Dictionnaires** : LibreOffice (GitHub `LibreOffice/dictionaries`) :
- FR : `fr_FR/fr.aff` + `fr_FR/fr.dic`
- EN : `en/en_US.aff` + `en/en_US.dic` (aussi en_GB, en_AU, en_CA, en_ZA)

**Limitations** : suggestions phon\u00e9tiques pas impl\u00e9ment\u00e9es, compounding complexe partiel.

### zspell (\u00e9cart\u00e9)

[GitHub](https://github.com/pluots/zspell) | v0.5.5 (juin 2024)

- Check stable, mais **suggestions instables** (derri\u00e8re feature flag `unstable-suggestions`, lentes)
- Derni\u00e8re release il y a 2 ans, d\u00e9veloppement lent
- **Verdict** : \u274c \u00c9cart\u00e9 au profit de spellbook

### Autres options

| Crate | Type | FR | Notes |
|---|---|---|---|
| `hunspell-rs` | FFI Hunspell C | Oui | D\u00e9pendance C, pas pure Rust |
| `symspell` | Symmetric delete | Oui (dict custom) | Rapide mais pas Hunspell-compatible |

---

## 2. Mod\u00e8les T5/GEC — \u00c9tat des lieux

### Mod\u00e8les actuels dans le projet

| Mod\u00e8le | ID catalogue | Params r\u00e9els | Taille | Langues | Sp\u00e9cialit\u00e9 |
|---|---|---|---|---|---|
| **Unbabel/gec-t5_small** | `correction:gec-t5-small` | 60M | 242 MB | EN, CS, DE, RU (transfer multi) | GEC g\u00e9n\u00e9ral |
| **fdemelo/t5-base-spell-correction-fr** | `correction:t5-spell-fr` | 200M | 892 MB | FR | Orthographe FR |
| **morenolq/flanec-large-cd** | `correction:flanec-large` | ~800M | 990 MB | EN | Post-ASR (8 domaines) |
| **pszemraj/flan-t5-large-grammar-synthesis** | `correction:flan-t5-grammar` | ~800M | 3.1 GB | EN | Grammaire EN |

**Corrections** :
- `Unbabel/gec-t5_small` : le fran\u00e7ais n'est PAS dans les langues d'entra\u00eenement (EN/CS/DE/RU seulement) — le transfert multilingue peut aider mais n'est pas v\u00e9rifi\u00e9
- `flanec-large` : params = ~800M (pas 250M comme indiqu\u00e9 dans le code)
- `flan-t5-grammar` : params = ~800M (pas 780M) — fait doublon avec FlanEC, moins bien valid\u00e9

### Candidats identifi\u00e9s

| Mod\u00e8le | Source | Params | Langues | Int\u00e9r\u00eat | Verdict |
|---|---|---|---|---|---|
| **morenolq/flanec-base-cd** | HuggingFace | ~250M | EN | Post-ASR, quasi m\u00eame WER que large (9.8% vs 8.9%) | \u2705 **\u00c0 ajouter** |
| flexudy/t5-base-multi-sentence-doctor | HuggingFace | ~220M | EN/FR/DE | 150K phrases seulement, auteurs disent "fine-tune needed" | \u274c Skip |
| flexudy/t5-small-wav2vec2-grammar-fixer | HuggingFace | ~60M | EN | Sp\u00e9cifique Wav2Vec2 (all-caps) | \u274c Skip |
| sdadas/byt5-text-correction | HuggingFace | 300M | 16 langues | ByT5 (byte-level), ponctuation/diacritiques seulement | \u274c Overlap PCS |
| TeXlyre/grammar-t5-small-onnx | HuggingFace | ~60M | EN | ONNX pr\u00eat mais EN-only, doublon gec-t5-small | \u274c Skip |

### Recommandations mod\u00e8les

1. **Ajouter `flanec-base-cd`** (~250M) — excellent rapport qualit\u00e9/taille pour EN post-ASR
2. **Consid\u00e9rer retirer `flan-t5-grammar`** — 3.1 GB, doublon de FlanEC, pas de benchmark post-ASR
3. **Corriger les params** dans le code du catalogue
4. **Le gap fran\u00e7ais reste** — aucun bon mod\u00e8le T5 GEC FR n'existe sur HuggingFace

---

## 3. LanguageTool & rule-based

| Option | Avantages | Inconv\u00e9nients | Verdict |
|---|---|---|---|
| **LanguageTool API** | 31 langues, 6000+ r\u00e8gles EN | Rate limit, latence, internet requis | \u274c |
| **LanguageTool serveur local** | Offline, toutes les r\u00e8gles | JVM, 500MB+ RAM, memory leaks | \u274c |
| **nlprule** (crate Rust) | Port natif, pur Rust, rapide | EN/DE/ES seulement, **pas de FR**, abandonn\u00e9 (2021) | \u274c |
| **languagetool-rust** | Client HTTP async | Juste un wrapper, n\u00e9cessite serveur | \u274c |

**Verdict g\u00e9n\u00e9ral** : incompatible avec l'architecture (pas de JVM, pas de serveur). \u274c

---

## 4. Approches par cat\u00e9gorie

### Sequence tagging (non-autor\u00e9gressif, rapide)

| Approche | Vitesse | Langues | ONNX | Statut |
|---|---|---|---|---|
| **GECToR** (DeBERTa) | 10x T5 | EN only | Export possible | Pas de mod\u00e8le ONNX pr\u00eat |
| **PIE** | 5-15x T5 | EN only | TensorFlow | Recherche |

**Verdict** : rapide mais EN-only, effort d'int\u00e9gration \u00e9lev\u00e9. \u274c pour l'instant.

### Seq2seq (autor\u00e9gressif)

C'est ce qu'on utilise (T5). Le probl\u00e8me n'est pas les mod\u00e8les mais l'infra (Candle + Metal → crashes).

### Hybride (spell-check + ML)

**Approche recommand\u00e9e** : spellbook (~5ms) comme couche rapide, T5 pour les cas complexes.

---

## 5. Correction post-ASR — Recherche acad\u00e9mique

| Approche | R\u00e9sultat | Paper |
|---|---|---|
| **DARAG** | 8-30% WER improvement | arXiv:2410.13198 |
| **Conformer multi-candidat** | 21% WER reduction | arXiv:2409.09554 |
| **Flan-T5 fine-tun\u00e9 ASR** | WER 13.1% → 4.2% | EMNLP 2023 ("Whispering LLaMA") |
| **FlanEC** (notre mod\u00e8le) | 24.6% relative WER reduction | SLT 2024 |
| **GenSEC Challenge** (NVIDIA) | Benchmark post-ASR | 2024 |

**Insight cl\u00e9** : les mod\u00e8les GEC g\u00e9n\u00e9riques ne comprennent pas les erreurs ASR typiques (homophones, mots coup\u00e9s). Un fine-tuning sur des paires (ASR brut → texte correct) donne 20-70% de gain WER.

---

## 6. Conversion de mod\u00e8les & repo HuggingFace

### Effort de conversion

| Source → Cible | Effort | Outil | RAM n\u00e9cessaire |
|---|---|---|---|
| HF Transformers → ONNX | ~1h/mod\u00e8le | `optimum-cli export onnx` | ~2x taille mod\u00e8le |
| T5 seq2seq → ONNX | ~2h | `optimum-cli` | ~2x (g\u00e9n\u00e8re encoder + decoder + decoder_with_past) |
| Quantisation INT8 | ~30min | `onnxruntime.quantization` | ~2x |
| Quantisation FP16 | ~10min | `onnxruntime` | ~1.5x |

**Aucune GPU n\u00e9cessaire** — tout tourne sur CPU, m\u00eame sur un MacBook. Un T5-small (242 MB) se convertit en 1-2 minutes.

### Strat\u00e9gie repo HF

1. Cr\u00e9er `huggingface.co/jplot/jona-whisper-models`
2. Script de conversion : `scripts/convert_model.py` (HF → ONNX → quantise → upload)
3. **Avantages** :
   - Un seul runtime (ort + CoreML) — \u00e9limine Candle
   - Mod\u00e8les optimis\u00e9s (INT8/FP16)
   - Contr\u00f4le total, pas de d\u00e9pendance repos tiers
   - Ouvre le spectre : tout mod\u00e8le HF convertible en ONNX devient accessible
4. **Priorit\u00e9** : convertir les T5 correction d'abord (source des crashes Candle/Metal)

---

## 7. VAD Silero — \u00c0 jour

| V\u00e9rification | R\u00e9sultat |
|---|---|
| Mod\u00e8le actuel | `silero_vad.onnx`, 2.3 MB, opset 16 |
| Mod\u00e8le GitHub master | Hash MD5 identique — **d\u00e9j\u00e0 \u00e0 jour** (v6) |
| Variante ifless (v6.2) | `silero_vad_op18_ifless.onnx`, 2.8 MB, opset 18, 4 nodes — sans `If` ONNX |

Le mod\u00e8le ifless est l\u00e9g\u00e8rement plus gros mais plus compatible (pas de `If` conditionnel). Migration possible mais pas urgente.

---

## 8. Plan d'action

### Court terme

1. **Int\u00e9grer spellbook** — spell-check FR/EN comme \u00e9tape pipeline (entre ponctuation et correction)
2. **Ajouter flanec-base-cd** — mod\u00e8le correction EN l\u00e9ger (250M)
3. **Corriger les param counts** dans le catalogue correction

### Moyen terme

4. **Cr\u00e9er le repo HF** + script de conversion
5. **Convertir T5 → ONNX** — \u00e9liminer Candle, unifier sur ort+CoreML
6. **Recherche \u00e9largie** — explorer ModelScope, ONNX Zoo, NGC, Papers with Code pour des mod\u00e8les FR

### Long terme

7. **Fine-tuner un T5-small sur des paires ASR→correct** pour FR (meilleur ROI pour le fran\u00e7ais)
8. **Explore GECToR** si des mod\u00e8les multilingues apparaissent

---

## R\u00e9f\u00e9rences cl\u00e9s

- [A Simple Recipe for Multilingual GEC](https://arxiv.org/abs/2106.03830) — Unbabel, 2021
- [OmniGEC Dataset](https://arxiv.org/abs/2509.14504) — 11 langues, 2025
- [DARAG: Post-ASR Correction](https://arxiv.org/abs/2410.13198) — 2024
- [FlanEC: SLT 2024](https://arxiv.org/abs/2409.09554) — Post-ASR error correction
- [GenSEC Challenge](https://research.nvidia.com/publication/2024-12_large-language-model-based-generative-error-correction-challenge-and-baselines) — NVIDIA
- [MultiGEC-2025 Shared Task](https://spraakbanken.github.io/multigec-2025/)
- [GECToR](https://arxiv.org/abs/2005.12592) — Grammarly
- [CoEdIT / mEdIT](https://arxiv.org/abs/2305.09857) — Google
- [spellbook](https://github.com/helix-editor/spellbook) — Pure Rust spell checker (Helix)
- [zspell](https://github.com/pluots/zspell) — Pure Rust spell checker (\u00e9cart\u00e9)
- [nlprule](https://github.com/bminixhofer/nlprule) — Rust LanguageTool port (abandonn\u00e9)
- [LibreOffice dictionaries](https://github.com/LibreOffice/dictionaries) — Hunspell FR/EN
- [Silero VAD](https://github.com/snakers4/silero-vad) — Voice Activity Detection

---

*Derni\u00e8re mise \u00e0 jour : mars 2026*
