# Recherche — Correction grammaticale, spell-check & post-ASR

*Mars 2026 — Synthèse complète de la recherche pour améliorer le pipeline de correction texte FR/EN.*

---

## Contexte

Le pipeline actuel :
- **Ponctuation** : PCS (47 langues, ONNX+CoreML) ou BERT (4 langues, ONNX+CoreML) — stable, rapide
- **Correction T5** : 4 modèles via Candle (safetensors + Metal) — problèmes de stabilité (boucles, crashes Metal)
- **LLM** : local (llama-cpp) ou cloud (OpenAI/Anthropic) — lourd mais flexible

Objectif : trouver des alternatives plus fiables, rapides, et couvrant bien FR + EN.

---

## 1. Spell-checking — Options Rust

### 1.1 spellbook (Hunspell-compatible)

[GitHub](https://github.com/helix-editor/spellbook) | [crates.io](https://crates.io/crates/spellbook) | [docs.rs](https://docs.rs/spellbook)

| Propriété | Valeur |
|---|---|
| Version | 0.4.0 |
| Maintenu par | Équipe Helix editor |
| Base | Réécriture Rust de Nuspell (successeur C++ de Hunspell) |
| Dépendances | `hashbrown` uniquement, `no_std` |
| Suggestions | Fonctionnelles (depuis v0.2.0), même algo que Nuspell |
| Statut | Alpha (API peut changer), tests Hunspell corpus passent |

```rust
let dict = spellbook::Dictionary::new(&aff, &dic)?;
dict.check("bonjour");  // true — vérification
// Suggestions disponibles via Suggester
```

**Dictionnaires LibreOffice** : `fr_FR/fr.aff` + `fr_FR/fr.dic`, `en/en_US.aff` + `en/en_US.dic`

### 1.2 SymSpell (algorithme symétrique delete)

[symspell crate](https://crates.io/crates/symspell) (reneklacan) | [symspell_rs](https://crates.io/crates/symspell_rs) (wolfgarbe officiel)

| Propriété | Valeur |
|---|---|
| Vitesse | **1 870x plus rapide que BK-tree**, sub-milliseconde par mot |
| Mémoire | ~50-100 MB pour 100K mots (edit distance 2) |
| API | `lookup` (mot unique), `lookup_compound` (multi-mots), `word_segmentation` |
| Dictionnaires | Format `mot\tfreq` — convertible depuis Lexique383 ou DELA |

**Avantage clé** : `lookup_compound` gère les erreurs de frontières de mots (fréquentes en ASR).

**3 crates Rust** : `symspell` (v0.4.5, WASM-compatible, `lookup_compound`), `symspell_rs` (v6.7.3, port officiel), `fast_symspell` (fork perf).

### 1.3 Comparaison

| Approche | Vitesse | Suggestions | Couverture FR | Effort intégration |
|---|---|---|---|---|
| **spellbook** | ~10ms/phrase | Oui (Nuspell) | Bonne (Hunspell) | Faible |
| **SymSpell** | <1ms/phrase | Oui (edit distance) | Excellente (DELA 683K) | Moyen (dict custom) |
| zspell | ~10ms/phrase | Instables | Bonne | ❌ Écarté |

### 1.4 Écartés

- **zspell** v0.5.5 — suggestions instables, développement lent (dernière release juin 2024)
- **hunspell-rs** — FFI C, pas pure Rust
- **NeuSpell** — Python/PyTorch, modèles BERT-sized, trop lourd

---

## 2. Dictionnaires & corpus français

### 2.1 DELA / Unitex (⭐ recommandé pour SymSpell)

[GitHub](https://github.com/HubTou/dict-fr-DELA)

| Propriété | Valeur |
|---|---|
| Formes fléchies | **683 824** (102 073 lemmes) |
| Composés | 108 436 entrées supplémentaires |
| Format | DELAF : `forme,lemme.cat+traits` |
| Encodage | UTF-16 LE |
| Date | 2006 mais complète pour le français standard |

Couverture quasi-totale des formes valides françaises. Idéal pour SymSpell.

### 2.2 Lexique 3.83 (⭐ recommandé pour fréquences)

[lexique.org](http://www.lexique.org/)

| Propriété | Valeur |
|---|---|
| Mots | **140 000** |
| Format | TSV, ~5 MB |
| Données | Forme, phonémique, lemme, syllabe, catégorie, genre, nombre, **fréquences** |
| Fréquences | 2 corpus (livres + sous-titres films) |

Les fréquences sont critiques pour le spell-check : préférer les mots courants.

### 2.3 GLAWI (Wiktionnaire)

[Site](http://redac.univ-tlse2.fr/lexiques/glawi.html) — 1 341 410 articles XML du Wiktionnaire. Massif mais lourd. Utile pour les transcriptions phonémiques (désambiguaïser les homophones).

### 2.4 Dictionnaires anglais

- **Hunspell (LibreOffice)** : `en_US.dic`/`.aff` — suffisant pour spellbook
- **SymSpell frequency lists** : [wolfgarbe/SymSpell](https://github.com/wolfgarbe/SymSpell) fournit des listes EN prêtes

---

## 3. Désambiguaïson phonétique (homophones)

Le problème central du post-ASR français : "vert", "verre", "ver", "vers" sont tous valides mais seul un est correct en contexte.

### 3.1 rphonetic (crate Rust)

[crates.io](https://crates.io/crates/rphonetic) — Port Rust d'Apache commons-codec v1.15

Algorithmes disponibles :
- **Phonex** — adapté au français (noms et mots généraux)
- **Beider-Morse** — multilingue avec détection de langue, règles FR
- **Double Metaphone** — 2 encodages par mot, multilingue
- Soundex, Refined Soundex, NYSIIS, Caverphone, Cologne, Daitch-Mokotoff

### 3.2 Sonnex (gap)

Meilleur algorithme phonétique FR spécifique. Encode les sons comme des chiffres ("on" = "3").
- Existe seulement en JavaScript ([Talisman](https://yomguithereal.github.io/talisman/phonetics/french)) et Haskell
- Port Rust faisable (~200 lignes, rule-based)

### 3.3 Stratégie désambiguaïson

```
mot ASR → encodage phonétique → groupe d'homophones
       → classement par fréquence (Lexique383) + contexte bigram
       → mot correct
```

---

## 4. N-gram Language Models (KenLM)

[GitHub](https://github.com/kpu/kenlm) | [Modèles FR HuggingFace](https://huggingface.co/edugp/kenlm)

| Propriété | Valeur |
|---|---|
| Vitesse | Sub-microseconde par requête (memory-mapped) |
| Modèle FR | Disponible (Wikipedia français), 50-500 MB selon pruning |
| Bindings Rust | **N'existent pas** — FFI à écrire |
| Usage | Rescoring de candidats, désambiguaïson homophones, correction frontières mots |

**Très prometteur** pour les homophones FR mais nécessite un travail FFI. Un 3-gram pruné ferait <50 MB.

---

## 5. Modèles T5/GEC — État des lieux

### 5.1 Modèles actuels dans le projet

| Modèle | ID catalogue | Params réels | Taille | Langues | Spécialité |
|---|---|---|---|---|---|
| **Unbabel/gec-t5_small** | `correction:gec-t5-small` | 60M | 242 MB | EN, CS, DE, RU | GEC général |
| **fdemelo/t5-base-spell-correction-fr** | `correction:t5-spell-fr` | 200M | 892 MB | FR | Orthographe FR |
| **morenolq/flanec-large-cd** | `correction:flanec-large` | ~800M | 990 MB | EN | Post-ASR (8 domaines) |
| **pszemraj/flan-t5-large-grammar-synthesis** | `correction:flan-t5-grammar` | ~800M | 3.1 GB | EN | Grammaire EN |

**Corrections nécessaires** :
- `gec-t5_small` : FR n'est **pas** dans les langues d'entraînement (EN/CS/DE/RU)
- `flanec-large` : params = ~800M (pas 250M dans le code)
- `flan-t5-grammar` : doublon de FlanEC, moins bien validé pour post-ASR

### 5.2 Candidats identifiés

| Modèle | Params | Langues | Intérêt | Verdict |
|---|---|---|---|---|
| **morenolq/flanec-base-cd** | ~250M | EN | Post-ASR, WER 9.8% (quasi = large) | ✅ À ajouter |
| flexudy/t5-base-multi-sentence-doctor | ~220M | EN/FR/DE | 150K phrases, "fine-tune needed" | ❌ |
| sdadas/byt5-text-correction | 300M | 16 langues | ByT5 (byte-level), ponctuation seulement | ❌ Overlap PCS |

### 5.3 Recherche élargie (hors HuggingFace)

| Source | Résultat |
|---|---|
| ONNX Model Zoo | **Aucun modèle GEC** (repo archivé juillet 2025) |
| TensorFlow Hub | **Aucun modèle GEC** |
| ModelScope | Que des LLMs généralistes |
| Kaggle | Datasets GEC FR, **pas de modèles** |
| NVIDIA NGC | Normalisation texte seulement, pas de GEC |
| Ollama/GGUF | `gnokit/improve-grammar` (Gemma-2B, EN), trop gros |
| Apple CoreML | Aucun modèle GEC pré-construit |
| OpenNMT | 1 modèle EN, hébergé sur HF |
| MultiGEC-2025 | **FR non inclus**, vainqueurs = LLaMA (trop gros) |
| GenSEC (NVIDIA) | = FlanEC (déjà dans le projet) |

**Constat** : tous les modèles GEC pré-entraînés avec poids téléchargeables sont sur HuggingFace. Aucune source alternative n'offre de modèle FR viable.

### 5.4 T5 via GGUF/llama.cpp (⭐ découverte)

T5 encoder-decoder **fonctionne maintenant dans llama.cpp** (PR #8055/#8141 merged) :
- `flan-t5-large-grammar-synthesis` GGUF existe déjà sur HF
- Quantisation GGUF (Q5_K_M) réduit la taille ~50%
- Alternative à Candle : utiliser llama-cpp-2 (déjà dans le projet pour LLM local)
- **Limitation** : pas d'imatrix pour T5, bindings Python instables (Rust via llama-cpp-2 non testé)

### 5.5 Autres outils français notés

| Outil | Type | FR | Notes |
|---|---|---|---|
| **Grammalecte** | Rule-based (Python) | ✅ | Correcteur typographique + grammaire, open-source, pas de modèle ML |
| **BARThez** | Seq2seq (BART) | ✅ | 216M params, modèle de langue FR général — pas fine-tuné GEC |
| **InstaCorrect** | Char seq2seq | ✅ | Code GitHub mais **pas de poids pré-entraînés** |
| **NeuroSpell** | DL corrector | ✅ | Commercial/closed-source |

---

## 6. LanguageTool & rule-based

| Option | FR | Avantages | Inconvénients | Verdict |
|---|---|---|---|---|
| LanguageTool API | ✅ | 31 langues, 6000+ règles | Rate limit, internet requis | ❌ |
| LanguageTool local | ✅ | Offline | JVM, 500MB+ RAM | ❌ |
| nlprule (Rust) | ❌ | Pur Rust, rapide | EN/DE/ES seulement, abandonné | ❌ |
| Grammalecte | ✅ | Règles FR de qualité | Python, pas de crate Rust | ❌ pour maintenant |

---

## 7. Correction post-ASR — Recherche académique

| Approche | Résultat | Paper |
|---|---|---|
| **DARAG** | 8-30% WER improvement | arXiv:2410.13198 |
| **Conformer multi-candidat** | 21% WER reduction | arXiv:2409.09554 |
| **Flan-T5 fine-tuné ASR** | WER 13.1% → 4.2% | EMNLP 2023 |
| **FlanEC** (notre modèle) | 24.6% relative WER reduction | SLT 2024 |
| **GenSEC Challenge** (NVIDIA) | Benchmark post-ASR | 2024 |
| **Calm-Whisper** | -84.5% hallucinations via 3 heads attention | 2024 |

**Insight clé** : les modèles GEC génériques ratent les erreurs ASR typiques. Un fine-tuning sur des paires (ASR brut → texte correct) donne 20-70% de gain WER.

---

## 8. Conversion de modèles & repo HuggingFace

### 8.1 Effort de conversion

| Source → Cible | Effort | Outil | RAM |
|---|---|---|---|
| HF Transformers → ONNX | ~1h/modèle | `optimum-cli export onnx` | ~2x taille |
| T5 seq2seq → ONNX | ~2h | `optimum-cli` | ~2x (3 fichiers) |
| Quantisation INT8 | ~30min | `onnxruntime.quantization` | ~2x |
| T5 → GGUF | ~1h | `llama.cpp/convert` | ~2x |

**Aucune GPU nécessaire** — CPU MacBook suffit.

### 8.2 Conversion réalisée

Les 4 modèles T5 correction ont été convertis en ONNX via `optimum-cli export onnx` (Python 3.13 + venv) :

| Modèle | Encoder | Decoder (merged) | Total | Repo HF |
|---|---|---|---|---|
| gec-t5-small | 135 MB | 222 MB | ~357 MB | `realjPlot/jonawhisper-gec-t5-small-onnx` |
| t5-spell-fr | 419 MB | 621 MB | ~1.0 GB | `realjPlot/jonawhisper-t5-spell-fr-onnx` |
| flanec-base | 419 MB | 622 MB | ~1.0 GB | `realjPlot/jonawhisper-flanec-base-onnx` |
| flanec-large | 1.3 GB | 1.8 GB | ~3.1 GB | `realjPlot/jonawhisper-flanec-large-onnx` |

**Note** : on utilise `decoder_model_merged.onnx` (combine decoder + decoder_with_past en un seul fichier) pour simplifier l'inférence.

### 8.3 Prochaine étape

1. Intégrer ces modèles ONNX dans le runtime ort+CoreML (remplacer Candle/safetensors)
2. Quantisation INT8 optionnelle pour réduire la taille
3. **Avantages** : un seul runtime, CoreML GPU, pas de crashes Metal/Candle

---

## 9. VAD Silero — À jour

| Vérification | Résultat |
|---|---|
| Modèle actuel | `silero_vad.onnx`, 2.3 MB, opset 16, v6 |
| Hash MD5 | Identique à GitHub master — **déjà à jour** |
| Variante ifless (v6.2) | 2.8 MB, opset 18, 4 nodes, sans `If` ONNX |

Migration ifless possible mais pas urgente.

---

## 10. Architecture proposée — Pipeline correction rapide

Pipeline 3 couches, <50ms total, sans GPU :

```
Étape 1 : SymSpell (<1ms)
  └─ Dict DELA (683K formes FR) ou Hunspell EN
  └─ lookup_compound pour erreurs frontières mots
  └─ Corrige les fautes évidentes (edit distance ≤2)

Étape 2 : Désambiguaïson phonétique (<1ms)
  └─ rphonetic (Phonex/Beider-Morse) : groupe homophones
  └─ Lexique383 fréquences : classement par usage
  └─ Contexte bigram simple pour choisir

Étape 3 (optionnel) : KenLM rescoring (<1ms)
  └─ Modèle n-gram FR (Wikipedia, <50 MB)
  └─ Score de fluence pour départager
  └─ Nécessite FFI C++ (pas de bindings Rust)
```

Ce pipeline serait une **étape entre ponctuation et T5/LLM** — rapide, légère, complémentaire.

---

## 11. Plan d'action

### Court terme (✅ fait)

1. ~~**Intégrer spellbook**~~ — spell-check FR/EN comme étape pipeline ✅
2. ~~**Ajouter flanec-base-cd**~~ — modèle correction EN léger (250M) ✅
3. ~~**Corriger param counts**~~ dans le catalogue correction ✅
4. ~~**Créer le repo HuggingFace**~~ — `realjPlot/jonawhisper-*-onnx` ✅
5. ~~**Convertir T5 → ONNX**~~ — 4 modèles convertis et uploadés ✅

### Moyen terme

6. **Intégrer T5 ONNX** — remplacer Candle/safetensors par ort+CoreML
7. **SymSpell + DELA** — spell-check haute performance FR (683K formes)
8. **rphonetic** — désambiguaïsation homophones FR
9. **Tester T5 via GGUF/llama.cpp** — alternative à Candle via llama-cpp-2

### Long terme

9. **Fine-tuner T5-small sur paires ASR→correct** pour FR
10. **KenLM FFI** — n-gram rescoring FR
11. **Porter Sonnex en Rust** — meilleur algo phonétique FR

---

## Références

### Modèles & datasets
- [Unbabel/gec-t5_small](https://huggingface.co/Unbabel/gec-t5_small) — T5 GEC multilingue
- [morenolq/flanec-large-cd](https://huggingface.co/morenolq/flanec-large-cd) — Post-ASR correction
- [morenolq/flanec-base-cd](https://huggingface.co/morenolq/flanec-base-cd) — Post-ASR correction (léger)
- [fdemelo/t5-base-spell-correction-fr](https://huggingface.co/fdemelo/t5-base-spell-correction-fr) — Spell FR
- [French GEC Dataset (Kaggle)](https://www.kaggle.com/datasets/isakbiderre/french-gec-dataset)
- [BARThez](https://github.com/moussaKam/BARThez) — Modèle langue FR (pas GEC)

### Spell-check & dictionnaires
- [spellbook](https://github.com/helix-editor/spellbook) — Rust Hunspell-compatible (⭐)
- [SymSpell](https://github.com/wolfgarbe/SymSpell) — Algorithme + dictionnaires
- [symspell crate](https://crates.io/crates/symspell) — Implémentation Rust
- [DELA / Unitex](https://github.com/HubTou/dict-fr-DELA) — 683K formes FR
- [Lexique 3.83](http://www.lexique.org/) — 140K mots FR + fréquences
- [GLAWI](http://redac.univ-tlse2.fr/lexiques/glawi.html) — Wiktionnaire structuré
- [LibreOffice dictionaries](https://github.com/LibreOffice/dictionaries) — Hunspell FR/EN

### Phonétique & n-grams
- [rphonetic](https://crates.io/crates/rphonetic) — Phonex/Beider-Morse/Metaphone Rust
- [Talisman Sonnex](https://yomguithereal.github.io/talisman/phonetics/french) — Phonétique FR (JS)
- [KenLM](https://github.com/kpu/kenlm) — N-gram LM
- [KenLM FR (HuggingFace)](https://huggingface.co/edugp/kenlm) — Modèles n-gram FR

### Papers
- [A Simple Recipe for Multilingual GEC](https://arxiv.org/abs/2106.03830) — Unbabel
- [DARAG: Post-ASR Correction](https://arxiv.org/abs/2410.13198)
- [FlanEC (SLT 2024)](https://arxiv.org/abs/2409.09554)
- [GenSEC Challenge](https://research.nvidia.com/publication/2024-12_large-language-model-based-generative-error-correction-challenge-and-baselines)
- [MultiGEC-2025](https://spraakbanken.github.io/multigec-2025/)
- [GECToR](https://arxiv.org/abs/2005.12592)
- [OmniGEC](https://arxiv.org/abs/2509.14504)

### Outils écartés
- [zspell](https://github.com/pluots/zspell) — Suggestions instables
- [nlprule](https://github.com/bminixhofer/nlprule) — Abandonné, pas de FR
- [Grammalecte](https://www.grammalecte.net/) — Python, pas de crate Rust
- [NeuroSpell](https://neurospell.com/) — Commercial/closed-source
- [DeepPavlov](https://docs.deeppavlov.ai/) — RU/EN seulement, trop gros

---

*Dernière mise à jour : mars 2026*
