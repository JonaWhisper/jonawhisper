# Pipeline Post-traitement Texte

Architecture du pipeline texte entre la sortie ASR brute et le texte final collé dans l'application cible.

---

## Vue d'ensemble

```
Pipeline actuel :
  ASR brut → [1. Hallucinations] → [2. Dictée] → [4/5. Punct OU Correct OU LLM] → [7. Finalize] → Paste

Pipeline proposé (complet) :
  ASR brut → [1. Hallucinations] → [2. Dictée] → [3. Disfluences]
           → [4. Ponctuation] → [5. Correction] → [6. ITN] → [7. Finalize] → Paste
```

**Différence clé** : actuellement les étapes 4 et 5 sont mutuellement exclusives (l'utilisateur choisit ponctuation OU correction OU LLM). Le pipeline proposé les chaîne séquentiellement.

---

## Étape 1 — Filtre hallucinations (✅ Implémenté)

**Fichier** : `post_processor.rs` — fonction `preprocess()` → `strip_hallucinations()`

**But** : supprimer les phrases parasites que Whisper génère sur du silence, du bruit ou des segments très courts.

**Mécanisme** :
- Liste `HALLUCINATIONS` : 30+ patterns connus (compilés en regex case-insensitive via `LazyLock`)
- Deux passes :
  1. Match exact (texte entier = hallucination) → retourne `""` (discard complet)
  2. Match partiel (hallucination dans un texte plus long) → suppression inline
- Si le résultat est vide après filtrage → son `Basso` + pas de transcription

**Patterns couverts** :
- Sous-titrage (FR) : "sous-titrage société radio-canada", "sous-titres par", etc.
- Signature (EN) : "thank you for watching", "please subscribe", etc.
- Artefacts : "♪", "...", "…", "www.", "http"
- Salutations orphelines : "bye.", "au revoir.", "à bientôt."

**Latence** : ~0ms (regex pré-compilées)

---

## Étape 2 — Commandes dictée (✅ Implémenté)

**Fichier** : `post_processor.rs` — fonction `preprocess()` → `apply_dictation_commands()`

**But** : convertir les commandes vocales en caractères de ponctuation ou formatage.

**Mécanisme** :
- Deux jeux de regex pré-compilées : `DICTATION_COMMANDS_FR` (16 commandes) et `DICTATION_COMMANDS_EN` (19 commandes)
- Détection de langue automatique (`resolve_language()`) si `language == "auto"` : compte les mots français courants, ≥2 → FR
- Substitution séquentielle (ordre important : "point d'interrogation" avant "point")

**Commandes supportées** :

| FR | EN | Résultat |
|---|---|---|
| point d'interrogation | question mark | `?` |
| point d'exclamation | exclamation mark | `!` |
| points de suspension | ellipsis | `…` |
| point-virgule | semicolon | `;` |
| deux-points / deux points | colon | `:` |
| ouvrir la parenthèse | open parenthesis | `(` |
| fermer la parenthèse | close parenthesis | `)` |
| ouvrir les guillemets | open quote | `«\u{00A0}` / `"` |
| fermer les guillemets | close quote | `\u{00A0}»` / `"` |
| à la ligne / nouvelle ligne | new line | `\n` |
| nouveau paragraphe | new paragraph | `\n\n` |
| virgule | comma | `,` |
| point | period / full stop | `.` |
| tiret | dash / hyphen | `-` |

**Latence** : ~0ms (regex pré-compilées)

---

## Étape 3 — Suppression disfluences (❌ Non implémenté)

**But** : supprimer les mots parasites (fillers) de l'oral qui polluent le texte écrit.

**Approche recommandée** : regex simple sur les fillers isolés.

### Fillers cibles

| Langue | Fillers | Contexte |
|---|---|---|
| FR | euh, heu, hum, bah, ben, beh | Hésitation pure |
| FR | enfin, quoi, genre, voilà, du coup, en fait, tu vois | Fillers discursifs (prudence — sens réel possible) |
| EN | uh, um, hmm | Hésitation pure |
| EN | like, you know, I mean, basically, actually, so, well, right | Fillers discursifs (prudence) |

### Stratégie de suppression

1. **Fillers purs** (euh, uh, um, hum, hmm) : suppression systématique en début/milieu de phrase — aucune ambiguïté
2. **Fillers discursifs** (genre, du coup, like, you know) : suppression uniquement quand isolés (entourés de pauses/ponctuation) — ces mots ont un sens réel dans certains contextes
3. **Regex word-boundary** : `\b(euh|heu|hum)\b` avec gestion des espaces résiduels

### Implémentation proposée

```rust
// Dans post_processor.rs, nouvelle fonction entre preprocess() et le cleanup model
static RE_FILLERS_FR: LazyLock<Regex> = LazyLock::new(||
    Regex::new(r"(?i)\b(euh|heu|hum|bah|ben|beh)\b\s*").unwrap()
);
static RE_FILLERS_EN: LazyLock<Regex> = LazyLock::new(||
    Regex::new(r"(?i)\b(uh|um|hmm)\b\s*").unwrap()
);
```

**Latence** : ~0ms

### Alternatives ML écartées

| Approche | Précision | Latence | Raison d'exclusion |
|---|---|---|---|
| CTC Forced Alignment (disfluency detection) | 81.6% | ~100ms | Nécessite modèle CTC + alignement mot-par-mot |
| Smooth-LLaMa (LLM fine-tuné) | ~85% | ~500ms | LLM dédié trop lourd pour un gain marginal |
| Whisper word timestamps + durée | Variable | ~0ms (post-hoc) | Nécessite word-level timestamps, pas disponible sur tous les engines |

---

## Étape 4 — Ponctuation & Capitalisation (✅ Implémenté)

**Fichiers** :
- `punct_common.rs` — logique partagée : `strip_and_split()`, `restore_punctuation_windowed()`, constantes `PUNCT_LABELS`, `WINDOW_SIZE` (230), `OVERLAP` (5)
- `bert_punctuation.rs` — inférence BERT via ort (ONNX Runtime + CoreML)
- `candle_punctuation.rs` — inférence BERT via Candle (safetensors + Metal GPU)
- `pcs_punctuation.rs` — inférence PCS (ONNX + SentencePiece protobuf → sliding window 128, overlap 16)
- `engines/bert.rs` — enregistrement des 2 modèles BERT
- `engines/pcs.rs` — enregistrement du modèle PCS

### Architecture commune (BERT)

```
Texte → strip_and_split() → fenêtres [230 mots, overlap 5]
     → tokenize (WordPiece) → inférence ONNX/Candle → labels par token
     → agrégation subwords → merge fenêtres → reconstruction texte ponctué
```

Labels prédits : `PUNCT_LABELS = ["", ".", ",", "?", "-", ":"]`

**Limitation BERT** : aucune prédiction de capitalisation. Seuls `.` `,` `?` `-` `:` sont restaurés.

### Architecture PCS

```
Texte → SentencePiece tokenize (Unigram, NFC+Lowercase+Metaspace)
     → fenêtres [128 tokens, overlap 16] → inférence ONNX 4 heads
     → post-punct | pre-punct | capitalization | segmentation
     → reconstruction texte ponctué + capitalisé
```

**4 heads** : pré-ponctuation (guillemets ouvrants, parenthèses), post-ponctuation (`.` `,` `?` `!`), capitalisation (UPPER/LOWER/ALL_CAPS), segmentation (SENTENCE_END).

**Tokenizer** : SentencePiece `.model` (protobuf) parsé via `prost` → construit programmatiquement en `tokenizers::Tokenizer` (Unigram + NFC + Lowercase + Metaspace) → caché comme `tokenizer.json`.

**Avantage PCS** : capitalisation native (les noms propres sont correctement capitalisés), 47 langues, plus compact que BERT.

### Modèles

| Modèle | Runtime | Taille | Langues | Capitalisation | Vitesse |
|---|---|---|---|---|---|
| Fullstop Large INT8 | ort + CoreML | 562 MB | 4 (FR/EN/DE/IT) | Non | ~100ms |
| Fullstop Base FP32 | Candle + Metal | 1.1 GB | 5 (+ NL) | Non | ~80ms |
| **PCS 47lang** | ort + CoreML | 233 MB | 47 | **Oui** | ~50ms |

**Dispatch** : `CleanupKind::Punctuation(runtime)` dans `engines/mod.rs`, routé dans `recording.rs`.

---

## Étape 5 — Correction grammaticale & orthographique (✅ Implémenté)

**Fichiers** :
- `t5_correction.rs` — `T5Context` : chargement safetensors + config.json + tokenizer.json, inférence Candle
- `engines/correction.rs` — enregistrement des 4 modèles T5

### Architecture

```
Texte → tokenize (SentencePiece via tokenizer.json)
     → T5 Encoder (safetensors, Metal GPU)
     → Décodage autorégressif (KV cache, greedy avec température 0.1)
     → Texte corrigé
```

### Anti-hallucination

Le décodage T5 autorégressif peut halluciner (répétitions, divergence). Protections implémentées :

| Protection | Mécanisme | Paramètre |
|---|---|---|
| Repeat penalty | Pénalise les tokens déjà générés | 1.1 |
| N-gram blocking | Interdit les n-grams déjà vus | Activé |
| Température | Réduit l'entropie de la distribution | 0.1 |
| Longueur max | Limite le nombre de tokens générés | Proportionnel à l'input |
| Sanitisation sortie | Vide → garder original, >3x input → garder original | Automatique |

### Modèles

| Modèle | Params | Taille | Langues | Spécialité | Vitesse |
|---|---|---|---|---|---|
| **GEC T5 Small** | 60M | 242 MB | 11 langues | GEC multilingue | ~200ms |
| T5 Spell FR | 220M | 892 MB | FR | Orthographe FR | ~500ms |
| FlanEC Large | 250M | 990 MB | EN | Post-ASR correction | ~800ms |
| Flan-T5 Grammar | 783M | 3.1 GB | EN | Grammaire EN | ~2s |

**Dispatch** : `CleanupKind::Correction` dans `engines/mod.rs`, routé dans `recording.rs` via `run_t5_correction()`.

---

## Étape 6 — ITN (Inverse Text Normalization) (❌ Non implémenté)

**But** : convertir les nombres, dates, heures et devises de leur forme orale vers leur forme écrite canonique.

### Catégories à couvrir

| Catégorie | Entrée (FR) | Sortie | Entrée (EN) | Sortie |
|---|---|---|---|---|
| Nombres cardinaux | vingt-trois | 23 | twenty three | 23 |
| Ordinaux | premier, deuxième | 1er, 2e | first, second | 1st, 2nd |
| Pourcentages | dix pour cent | 10% | ten percent | 10% |
| Heures | trois heures et quart | 3h15 | three fifteen | 3:15 |
| Devises | cinq euros cinquante | 5,50 € | five dollars | $5 |
| Dates | premier janvier | 1er janvier | January first | January 1st |
| Unités | deux kilomètres | 2 km | two miles | 2 mi |

### Implémentation proposée

Approche regex par couches, du plus spécifique au plus général :

```
1. Expressions composées : "vingt-trois" → 23 (lookup table 1-99)
2. Multiplicateurs : "deux cents" → 200, "trois mille" → 3000
3. Pourcentages : "{nombre} pour cent" → "{nombre}%"
4. Heures : "{nombre} heure(s) {nombre}" → "{H}h{MM}"
5. Devises : "{nombre} euros/dollars" → "{nombre} €/$"
6. Ordinaux : "premier" → "1er", "deuxième" → "2e"
```

**Complexité** : la composition FR ("quatre-vingt-dix-sept" = 97) nécessite une grammaire récursive, pas un simple lookup. Un parser dédié est recommandé.

**Latence estimée** : ~0ms (regex/lookup)

---

## Étape 7 — Finalize (✅ Implémenté)

**Fichier** : `post_processor.rs` — fonction `finalize()`

**But** : corriger l'espacement autour de la ponctuation et capitaliser le texte.

**Traitements** :

| Traitement | Regex | Exemple |
|---|---|---|
| Supprimer espace avant fermeture | `\s+([.,?!;:…)»"\]])` | `mot .` → `mot.` |
| Ajouter espace après ponctuation | `([.,?!;:…])([^\s\n…)»"\]\d])` | `mot.Mot` → `mot. Mot` |
| Supprimer espace après ouverture | `([\(«"\[)\s+` | `( mot` → `(mot` |
| Capitaliser après phrase | `([.?!]\s+\|\n)(\p{Ll})` | `fin. début` → `fin. Début` |
| Capitaliser premier caractère | Code direct | `bonjour` → `Bonjour` |

**Note** : `finalize()` est appelé **après** l'étape de cleanup (punct/correction/LLM), sauf pour le LLM où il est appelé **avant** (le LLM gère lui-même la ponctuation finale).

**Latence** : ~0ms (regex pré-compilées)

---

## Limitation actuelle : pas de chaînage

Le pipeline actuel dans `recording.rs` (`process_next_in_queue()`, lignes 362-492) utilise un `match` exclusif sur `CleanupKind` :

```
match cleanup_kind {
    Correction     → preprocess → T5 → finalize
    Punctuation(_) → preprocess → BERT/PCS → finalize
    LocalLlm       → preprocess → finalize → LLM
    CloudLlm(_)    → preprocess → finalize → LLM
    None           → preprocess → finalize
}
```

**Conséquence** : on ne peut pas chaîner ponctuation + correction. L'utilisateur choisit l'un OU l'autre dans les paramètres.

**Solution proposée** : séparer le choix ponctuation et correction en deux paramètres indépendants, et exécuter séquentiellement : `preprocess → disfluences → ponctuation → correction → ITN → finalize`.

---

## Fichiers concernés

| Fichier | Rôle dans le pipeline |
|---|---|
| `recording.rs` | Orchestration : `process_next_in_queue()` appelle les étapes dans l'ordre |
| `post_processor.rs` | Étapes 1 (hallucinations), 2 (dictée), 7 (finalize) |
| `punct_common.rs` | Logique partagée ponctuation : windowing, labels, strip_and_split |
| `bert_punctuation.rs` | Étape 4 : inférence BERT ort (ONNX + CoreML) |
| `candle_punctuation.rs` | Étape 4 : inférence BERT Candle (safetensors + Metal) |
| `pcs_punctuation.rs` | Étape 4 : inférence PCS (ONNX + SentencePiece, 4 heads) |
| `t5_correction.rs` | Étape 5 : T5Context, chargement modèle, décodage autorégressif |
| `ort_session.rs` | Builder de session ort partagé (CoreML EP) — utilisé par BERT, PCS |
| `engines/mod.rs` | `CleanupKind` enum, dispatch, `EngineCatalog` |
| `engines/bert.rs` | Enregistrement modèles BERT punctuation (2 modèles) |
| `engines/pcs.rs` | Enregistrement modèle PCS punctuation (1 modèle, 47 langues) |
| `engines/correction.rs` | Enregistrement modèles T5 correction (4 modèles) |
| `state.rs` | Préférences : `cleanup_model_id`, `text_cleanup_enabled` |

---

## Roadmap

### Phase 1 — Quick wins (effort très faible)

1. **Regex disfluences** FR/EN dans `post_processor.rs` — nouvelle fonction `strip_fillers()` entre `preprocess()` et le modèle de cleanup
2. **Regex ITN basique** — nombres 1-99 FR/EN, pourcentages, heures simples

### Phase 2 — Améliorations modèles (effort modéré)

3. **Chaînage ponctuation + correction** — séparer les paramètres, exécuter séquentiellement dans `recording.rs`
4. **Évaluer GECToR** — ONNX export du modèle Grammarly, tag-based = 10x plus rapide que T5, pas de hallucination
5. **Évaluer Harper** — `harper-core` crate, rule-based EN, <10ms, complémentaire aux modèles ML

### Phase 3 — Pipeline avancé (effort élevé)

6. **ITN parser FR** — grammaire récursive pour nombres composés ("quatre-vingt-dix-sept")
7. **GECToR multilingue** — entraîner/trouver un modèle tag-based FR
8. **Truecasing avancé** — au-delà de PCS, utiliser des modèles NER pour les noms propres non couverts

---

*Dernière mise à jour : mars 2026*
