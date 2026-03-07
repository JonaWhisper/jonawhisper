# Recherche — Correction grammaticale, spell-check & post-ASR

*Mars 2026 — Synth\u00e8se compl\u00e8te de la recherche pour am\u00e9liorer le pipeline de correction texte FR/EN.*

---

## Contexte

Le pipeline actuel :
- **Ponctuation** : PCS (47 langues, ONNX+CoreML) ou BERT (4 langues, ONNX+CoreML) — stable, rapide
- **Correction T5** : 4 mod\u00e8les via Candle (safetensors + Metal) — probl\u00e8mes de stabilit\u00e9 (boucles, crashes Metal)
- **LLM** : local (llama-cpp) ou cloud (OpenAI/Anthropic) — lourd mais flexible

Objectif : trouver des alternatives plus fiables, rapides, et couvrant bien FR + EN.

---

## 1. Spell-checking — Options Rust

### 1.1 spellbook (Hunspell-compatible)

[GitHub](https://github.com/helix-editor/spellbook) | [crates.io](https://crates.io/crates/spellbook) | [docs.rs](https://docs.rs/spellbook)

| Propri\u00e9t\u00e9 | Valeur |
|---|---|
| Version | 0.3.4 (avril 2025) |
| Maintenu par | \u00c9quipe Helix editor |
| Base | R\u00e9\u00e9criture Rust de Nuspell (successeur C++ de Hunspell) |
| D\u00e9pendances | `hashbrown` uniquement, `no_std` |
| Suggestions | Fonctionnelles (depuis v0.2.0), m\u00eame algo que Nuspell |
| Statut | Alpha (API peut changer), tests Hunspell corpus passent |

```rust
let dict = spellbook::Dictionary::new(&aff, &dic)?;
dict.check("bonjour");  // true — v\u00e9rification
// Suggestions disponibles via Suggester
```

**Dictionnaires LibreOffice** : `fr_FR/fr.aff` + `fr_FR/fr.dic`, `en/en_US.aff` + `en/en_US.dic`

### 1.2 SymSpell (algorithme sym\u00e9trique delete)

[symspell crate](https://crates.io/crates/symspell) (reneklacan) | [symspell_rs](https://crates.io/crates/symspell_rs) (wolfgarbe officiel)

| Propri\u00e9t\u00e9 | Valeur |
|---|---|
| Vitesse | **1 870x plus rapide que BK-tree**, sub-milliseconde par mot |
| M\u00e9moire | ~50-100 MB pour 100K mots (edit distance 2) |
| API | `lookup` (mot unique), `lookup_compound` (multi-mots), `word_segmentation` |
| Dictionnaires | Format `mot\tfreq` — convertible depuis Lexique383 ou DELA |

**Avantage cl\u00e9** : `lookup_compound` g\u00e8re les erreurs de fronti\u00e8res de mots (fr\u00e9quentes en ASR).

**3 crates Rust** : `symspell` (v0.4.5, WASM-compatible, `lookup_compound`), `symspell_rs` (v6.7.3, port officiel), `fast_symspell` (fork perf).

### 1.3 Comparaison

| Approche | Vitesse | Suggestions | Couverture FR | Effort int\u00e9gration |
|---|---|---|---|---|
| **spellbook** | ~10ms/phrase | Oui (Nuspell) | Bonne (Hunspell) | Faible |
| **SymSpell** | <1ms/phrase | Oui (edit distance) | Excellente (DELA 683K) | Moyen (dict custom) |
| zspell | ~10ms/phrase | Instables | Bonne | \u274c \u00c9cart\u00e9 |

### 1.4 \u00c9cart\u00e9s

- **zspell** v0.5.5 — suggestions instables, d\u00e9veloppement lent (derni\u00e8re release juin 2024)
- **hunspell-rs** — FFI C, pas pure Rust
- **NeuSpell** — Python/PyTorch, mod\u00e8les BERT-sized, trop lourd

---

## 2. Dictionnaires & corpus fran\u00e7ais

### 2.1 DELA / Unitex (\u2b50 recommand\u00e9 pour SymSpell)

[GitHub](https://github.com/HubTou/dict-fr-DELA)

| Propri\u00e9t\u00e9 | Valeur |
|---|---|
| Formes fl\u00e9chies | **683 824** (102 073 lemmes) |
| Compos\u00e9s | 108 436 entr\u00e9es suppl\u00e9mentaires |
| Format | DELAF : `forme,lemme.cat+traits` |
| Encodage | UTF-16 LE |
| Date | 2006 mais compl\u00e8te pour le fran\u00e7ais standard |

Couverture quasi-totale des formes valides fran\u00e7aises. Id\u00e9al pour SymSpell.

### 2.2 Lexique 3.83 (\u2b50 recommand\u00e9 pour fr\u00e9quences)

[lexique.org](http://www.lexique.org/)

| Propri\u00e9t\u00e9 | Valeur |
|---|---|
| Mots | **140 000** |
| Format | TSV, ~5 MB |
| Donn\u00e9es | Forme, phon\u00e9mique, lemme, syllabe, cat\u00e9gorie, genre, nombre, **fr\u00e9quences** |
| Fr\u00e9quences | 2 corpus (livres + sous-titres films) |

Les fr\u00e9quences sont critiques pour le spell-check : pr\u00e9f\u00e9rer les mots courants.

### 2.3 GLAWI (Wiktionnaire)

[Site](http://redac.univ-tlse2.fr/lexiques/glawi.html) — 1 341 410 articles XML du Wiktionnaire. Massif mais lourd. Utile pour les transcriptions phon\u00e9miques (d\u00e9sambigua\u00efser les homophones).

### 2.4 Dictionnaires anglais

- **Hunspell (LibreOffice)** : `en_US.dic`/`.aff` — suffisant pour spellbook
- **SymSpell frequency lists** : [wolfgarbe/SymSpell](https://github.com/wolfgarbe/SymSpell) fournit des listes EN pr\u00eates

---

## 3. D\u00e9sambigua\u00efson phon\u00e9tique (homophones)

Le probl\u00e8me central du post-ASR fran\u00e7ais : "vert", "verre", "ver", "vers" sont tous valides mais seul un est correct en contexte.

### 3.1 rphonetic (crate Rust)

[crates.io](https://crates.io/crates/rphonetic) — Port Rust d'Apache commons-codec v1.15

Algorithmes disponibles :
- **Phonex** — adapt\u00e9 au fran\u00e7ais (noms et mots g\u00e9n\u00e9raux)
- **Beider-Morse** — multilingue avec d\u00e9tection de langue, r\u00e8gles FR
- **Double Metaphone** — 2 encodages par mot, multilingue
- Soundex, Refined Soundex, NYSIIS, Caverphone, Cologne, Daitch-Mokotoff

### 3.2 Sonnex (gap)

Meilleur algorithme phon\u00e9tique FR sp\u00e9cifique. Encode les sons comme des chiffres ("on" = "3").
- Existe seulement en JavaScript ([Talisman](https://yomguithereal.github.io/talisman/phonetics/french)) et Haskell
- Port Rust faisable (~200 lignes, rule-based)

### 3.3 Strat\u00e9gie d\u00e9sambigua\u00efson

```
mot ASR → encodage phon\u00e9tique → groupe d'homophones
       → classement par fr\u00e9quence (Lexique383) + contexte bigram
       → mot correct
```

---

## 4. N-gram Language Models (KenLM)

[GitHub](https://github.com/kpu/kenlm) | [Mod\u00e8les FR HuggingFace](https://huggingface.co/edugp/kenlm)

| Propri\u00e9t\u00e9 | Valeur |
|---|---|
| Vitesse | Sub-microseconde par requ\u00eate (memory-mapped) |
| Mod\u00e8le FR | Disponible (Wikipedia fran\u00e7ais), 50-500 MB selon pruning |
| Bindings Rust | **N'existent pas** — FFI \u00e0 \u00e9crire |
| Usage | Rescoring de candidats, d\u00e9sambigua\u00efson homophones, correction fronti\u00e8res mots |

**Tr\u00e8s prometteur** pour les homophones FR mais n\u00e9cessite un travail FFI. Un 3-gram prun\u00e9 ferait <50 MB.

---

## 5. Mod\u00e8les T5/GEC — \u00c9tat des lieux

### 5.1 Mod\u00e8les actuels dans le projet

| Mod\u00e8le | ID catalogue | Params r\u00e9els | Taille | Langues | Sp\u00e9cialit\u00e9 |
|---|---|---|---|---|---|
| **Unbabel/gec-t5_small** | `correction:gec-t5-small` | 60M | 242 MB | EN, CS, DE, RU | GEC g\u00e9n\u00e9ral |
| **fdemelo/t5-base-spell-correction-fr** | `correction:t5-spell-fr` | 200M | 892 MB | FR | Orthographe FR |
| **morenolq/flanec-large-cd** | `correction:flanec-large` | ~800M | 990 MB | EN | Post-ASR (8 domaines) |
| **pszemraj/flan-t5-large-grammar-synthesis** | `correction:flan-t5-grammar` | ~800M | 3.1 GB | EN | Grammaire EN |

**Corrections n\u00e9cessaires** :
- `gec-t5_small` : FR n'est **pas** dans les langues d'entra\u00eenement (EN/CS/DE/RU)
- `flanec-large` : params = ~800M (pas 250M dans le code)
- `flan-t5-grammar` : doublon de FlanEC, moins bien valid\u00e9 pour post-ASR

### 5.2 Candidats identifi\u00e9s

| Mod\u00e8le | Params | Langues | Int\u00e9r\u00eat | Verdict |
|---|---|---|---|---|
| **morenolq/flanec-base-cd** | ~250M | EN | Post-ASR, WER 9.8% (quasi = large) | \u2705 \u00c0 ajouter |
| flexudy/t5-base-multi-sentence-doctor | ~220M | EN/FR/DE | 150K phrases, "fine-tune needed" | \u274c |
| sdadas/byt5-text-correction | 300M | 16 langues | ByT5 (byte-level), ponctuation seulement | \u274c Overlap PCS |

### 5.3 Recherche \u00e9largie (hors HuggingFace)

| Source | R\u00e9sultat |
|---|---|
| ONNX Model Zoo | **Aucun mod\u00e8le GEC** (repo archiv\u00e9 juillet 2025) |
| TensorFlow Hub | **Aucun mod\u00e8le GEC** |
| ModelScope | Que des LLMs g\u00e9n\u00e9ralistes |
| Kaggle | Datasets GEC FR, **pas de mod\u00e8les** |
| NVIDIA NGC | Normalisation texte seulement, pas de GEC |
| Ollama/GGUF | `gnokit/improve-grammar` (Gemma-2B, EN), trop gros |
| Apple CoreML | Aucun mod\u00e8le GEC pr\u00e9-construit |
| OpenNMT | 1 mod\u00e8le EN, h\u00e9berg\u00e9 sur HF |
| MultiGEC-2025 | **FR non inclus**, vainqueurs = LLaMA (trop gros) |
| GenSEC (NVIDIA) | = FlanEC (d\u00e9j\u00e0 dans le projet) |

**Constat** : tous les mod\u00e8les GEC pr\u00e9-entra\u00een\u00e9s avec poids t\u00e9l\u00e9chargeables sont sur HuggingFace. Aucune source alternative n'offre de mod\u00e8le FR viable.

### 5.4 T5 via GGUF/llama.cpp (\u2b50 d\u00e9couverte)

T5 encoder-decoder **fonctionne maintenant dans llama.cpp** (PR #8055/#8141 merged) :
- `flan-t5-large-grammar-synthesis` GGUF existe d\u00e9j\u00e0 sur HF
- Quantisation GGUF (Q5_K_M) r\u00e9duit la taille ~50%
- Alternative \u00e0 Candle : utiliser llama-cpp-2 (d\u00e9j\u00e0 dans le projet pour LLM local)
- **Limitation** : pas d'imatrix pour T5, bindings Python instables (Rust via llama-cpp-2 non test\u00e9)

### 5.5 Autres outils fran\u00e7ais not\u00e9s

| Outil | Type | FR | Notes |
|---|---|---|---|
| **Grammalecte** | Rule-based (Python) | \u2705 | Correcteur typographique + grammaire, open-source, pas de mod\u00e8le ML |
| **BARThez** | Seq2seq (BART) | \u2705 | 216M params, mod\u00e8le de langue FR g\u00e9n\u00e9ral — pas fine-tun\u00e9 GEC |
| **InstaCorrect** | Char seq2seq | \u2705 | Code GitHub mais **pas de poids pr\u00e9-entra\u00een\u00e9s** |
| **NeuroSpell** | DL corrector | \u2705 | Commercial/closed-source |

---

## 6. LanguageTool & rule-based

| Option | FR | Avantages | Inconv\u00e9nients | Verdict |
|---|---|---|---|---|
| LanguageTool API | \u2705 | 31 langues, 6000+ r\u00e8gles | Rate limit, internet requis | \u274c |
| LanguageTool local | \u2705 | Offline | JVM, 500MB+ RAM | \u274c |
| nlprule (Rust) | \u274c | Pur Rust, rapide | EN/DE/ES seulement, abandonn\u00e9 | \u274c |
| Grammalecte | \u2705 | R\u00e8gles FR de qualit\u00e9 | Python, pas de crate Rust | \u274c pour maintenant |

---

## 7. Correction post-ASR — Recherche acad\u00e9mique

| Approche | R\u00e9sultat | Paper |
|---|---|---|
| **DARAG** | 8-30% WER improvement | arXiv:2410.13198 |
| **Conformer multi-candidat** | 21% WER reduction | arXiv:2409.09554 |
| **Flan-T5 fine-tun\u00e9 ASR** | WER 13.1% \u2192 4.2% | EMNLP 2023 |
| **FlanEC** (notre mod\u00e8le) | 24.6% relative WER reduction | SLT 2024 |
| **GenSEC Challenge** (NVIDIA) | Benchmark post-ASR | 2024 |
| **Calm-Whisper** | -84.5% hallucinations via 3 heads attention | 2024 |

**Insight cl\u00e9** : les mod\u00e8les GEC g\u00e9n\u00e9riques ratent les erreurs ASR typiques. Un fine-tuning sur des paires (ASR brut \u2192 texte correct) donne 20-70% de gain WER.

---

## 8. Conversion de mod\u00e8les & repo HuggingFace

### 8.1 Effort de conversion

| Source \u2192 Cible | Effort | Outil | RAM |
|---|---|---|---|
| HF Transformers \u2192 ONNX | ~1h/mod\u00e8le | `optimum-cli export onnx` | ~2x taille |
| T5 seq2seq \u2192 ONNX | ~2h | `optimum-cli` | ~2x (3 fichiers) |
| Quantisation INT8 | ~30min | `onnxruntime.quantization` | ~2x |
| T5 \u2192 GGUF | ~1h | `llama.cpp/convert` | ~2x |

**Aucune GPU n\u00e9cessaire** — CPU MacBook suffit.

### 8.2 Strat\u00e9gie repo HF

1. Cr\u00e9er `huggingface.co/jplot/jona-whisper-models` (ou similaire)
2. Script `scripts/convert_model.py` : HF \u2192 ONNX \u2192 quantise \u2192 upload
3. **Avantages** : un seul runtime (ort+CoreML), mod\u00e8les optimis\u00e9s, contr\u00f4le total
4. **Priorit\u00e9** : convertir T5 correction (source des crashes Candle/Metal)

---

## 9. VAD Silero — \u00c0 jour

| V\u00e9rification | R\u00e9sultat |
|---|---|
| Mod\u00e8le actuel | `silero_vad.onnx`, 2.3 MB, opset 16, v6 |
| Hash MD5 | Identique \u00e0 GitHub master — **d\u00e9j\u00e0 \u00e0 jour** |
| Variante ifless (v6.2) | 2.8 MB, opset 18, 4 nodes, sans `If` ONNX |

Migration ifless possible mais pas urgente.

---

## 10. Architecture propos\u00e9e — Pipeline correction rapide

Pipeline 3 couches, <50ms total, sans GPU :

```
\u00c9tape 1 : SymSpell (<1ms)
  \u2514\u2500 Dict DELA (683K formes FR) ou Hunspell EN
  \u2514\u2500 lookup_compound pour erreurs fronti\u00e8res mots
  \u2514\u2500 Corrige les fautes \u00e9videntes (edit distance \u22642)

\u00c9tape 2 : D\u00e9sambigua\u00efson phon\u00e9tique (<1ms)
  \u2514\u2500 rphonetic (Phonex/Beider-Morse) : groupe homophones
  \u2514\u2500 Lexique383 fr\u00e9quences : classement par usage
  \u2514\u2500 Contexte bigram simple pour choisir

\u00c9tape 3 (optionnel) : KenLM rescoring (<1ms)
  \u2514\u2500 Mod\u00e8le n-gram FR (Wikipedia, <50 MB)
  \u2514\u2500 Score de fluence pour d\u00e9partager
  \u2514\u2500 N\u00e9cessite FFI C++ (pas de bindings Rust)
```

Ce pipeline serait une **\u00e9tape entre ponctuation et T5/LLM** — rapide, l\u00e9g\u00e8re, compl\u00e9mentaire.

---

## 11. Plan d'action

### Court terme

1. **Int\u00e9grer spellbook** — spell-check FR/EN comme \u00e9tape pipeline
2. **Ajouter flanec-base-cd** — mod\u00e8le correction EN l\u00e9ger (250M)
3. **Corriger param counts** dans le catalogue correction
4. **Cr\u00e9er le repo HuggingFace**

### Moyen terme

5. **SymSpell + DELA** — spell-check haute performance FR (683K formes)
6. **Convertir T5 \u2192 ONNX** — \u00e9liminer Candle, unifier sur ort+CoreML
7. **rphonetic** — d\u00e9sambigua\u00efson homophones FR
8. **Tester T5 via GGUF/llama.cpp** — alternative \u00e0 Candle via llama-cpp-2

### Long terme

9. **Fine-tuner T5-small sur paires ASR\u2192correct** pour FR
10. **KenLM FFI** — n-gram rescoring FR
11. **Porter Sonnex en Rust** — meilleur algo phon\u00e9tique FR

---

## R\u00e9f\u00e9rences

### Mod\u00e8les & datasets
- [Unbabel/gec-t5_small](https://huggingface.co/Unbabel/gec-t5_small) — T5 GEC multilingue
- [morenolq/flanec-large-cd](https://huggingface.co/morenolq/flanec-large-cd) — Post-ASR correction
- [morenolq/flanec-base-cd](https://huggingface.co/morenolq/flanec-base-cd) — Post-ASR correction (l\u00e9ger)
- [fdemelo/t5-base-spell-correction-fr](https://huggingface.co/fdemelo/t5-base-spell-correction-fr) — Spell FR
- [French GEC Dataset (Kaggle)](https://www.kaggle.com/datasets/isakbiderre/french-gec-dataset)
- [BARThez](https://github.com/moussaKam/BARThez) — Mod\u00e8le langue FR (pas GEC)

### Spell-check & dictionnaires
- [spellbook](https://github.com/helix-editor/spellbook) — Rust Hunspell-compatible (\u2b50)
- [SymSpell](https://github.com/wolfgarbe/SymSpell) — Algorithme + dictionnaires
- [symspell crate](https://crates.io/crates/symspell) — Impl\u00e9mentation Rust
- [DELA / Unitex](https://github.com/HubTou/dict-fr-DELA) — 683K formes FR
- [Lexique 3.83](http://www.lexique.org/) — 140K mots FR + fr\u00e9quences
- [GLAWI](http://redac.univ-tlse2.fr/lexiques/glawi.html) — Wiktionnaire structur\u00e9
- [LibreOffice dictionaries](https://github.com/LibreOffice/dictionaries) — Hunspell FR/EN

### Phon\u00e9tique & n-grams
- [rphonetic](https://crates.io/crates/rphonetic) — Phonex/Beider-Morse/Metaphone Rust
- [Talisman Sonnex](https://yomguithereal.github.io/talisman/phonetics/french) — Phon\u00e9tique FR (JS)
- [KenLM](https://github.com/kpu/kenlm) — N-gram LM
- [KenLM FR (HuggingFace)](https://huggingface.co/edugp/kenlm) — Mod\u00e8les n-gram FR

### Papers
- [A Simple Recipe for Multilingual GEC](https://arxiv.org/abs/2106.03830) — Unbabel
- [DARAG: Post-ASR Correction](https://arxiv.org/abs/2410.13198)
- [FlanEC (SLT 2024)](https://arxiv.org/abs/2409.09554)
- [GenSEC Challenge](https://research.nvidia.com/publication/2024-12_large-language-model-based-generative-error-correction-challenge-and-baselines)
- [MultiGEC-2025](https://spraakbanken.github.io/multigec-2025/)
- [GECToR](https://arxiv.org/abs/2005.12592)
- [OmniGEC](https://arxiv.org/abs/2509.14504)

### Outils \u00e9cart\u00e9s
- [zspell](https://github.com/pluots/zspell) — Suggestions instables
- [nlprule](https://github.com/bminixhofer/nlprule) — Abandonn\u00e9, pas de FR
- [Grammalecte](https://www.grammalecte.net/) — Python, pas de crate Rust
- [NeuroSpell](https://neurospell.com/) — Commercial/closed-source
- [DeepPavlov](https://docs.deeppavlov.ai/) — RU/EN seulement, trop gros

---

*Derni\u00e8re mise \u00e0 jour : mars 2026*
