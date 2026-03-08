# Pipeline Post-traitement Texte

Architecture du pipeline texte entre la sortie ASR brute et le texte final collé dans l'application cible.

---

## Vue d'ensemble

```
ASR brut → [1. Hallucinations] → [2. Dictée] → [3. Disfluences]
         → [4. Ponctuation] → [5. Spell-check] → [6. Correction/LLM] → [7. Finalize] → [8. ITN] → Paste
```

**Ponctuation et correction sont indépendants** : chacun a son propre paramètre et dropdown. L'utilisateur peut activer ponctuation seule, correction seule, ou les deux chaînés. Le LLM cloud reçoit `finalize()` en amont (il gère sa propre ponctuation).

---

## Étape 1 — Filtre hallucinations (✅ Implémenté)

**Fichier** : `cleanup/post_processor/hallucinations.rs` — fonction `strip_hallucinations()`

**But** : supprimer les phrases parasites que Whisper génère sur du silence, du bruit ou des segments très courts.

**Mécanisme** :
- Liste `HALLUCINATIONS` : 60+ patterns connus en 9 langues (compilés en regex case-insensitive via `LazyLock`)
- Trois passes :
  1. Détection musique/symboles uniquement (♪, ♫, …) → discard complet
  2. Match exact (texte entier = hallucination) → retourne `""` (discard complet)
  3. Détection répétition excessive (même mot 3× consécutifs, ou >70% d'un seul mot) → discard
  4. Match partiel (hallucination dans un texte plus long) → suppression inline
- Si le résultat est vide après filtrage → son `Basso` + pas de transcription

**Patterns couverts** (9 langues : FR, EN, DE, ES, PT, IT, NL, PL, RU) :
- Sous-titrage : "sous-titrage société radio-canada", "subtitles by", "Untertitel im Auftrag des ZDF", etc.
- Signature : "thank you for watching", "gracias por ver", "спасибо за просмотр", etc.
- Artefacts : "♪", "...", "…", "www.", "http"
- Salutations orphelines : "bye.", "au revoir.", "tschüss", "tchau", etc.

**Latence** : ~0ms (regex pré-compilées)

---

## Étape 2 — Commandes dictée (✅ Implémenté)

**Fichier** : `cleanup/post_processor/dictation.rs` — fonction `apply_dictation_commands()`

**But** : convertir les commandes vocales en caractères de ponctuation ou formatage.

**Mécanisme** :
- Deux jeux de regex pré-compilées : `DICTATION_COMMANDS_FR` (17 commandes) et `DICTATION_COMMANDS_EN` (21 commandes)
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

## Étape 3 — Suppression disfluences (✅ Implémenté)

**Fichier** : `cleanup/post_processor/fillers.rs` — fonction `strip_fillers()`

**But** : supprimer les mots parasites (fillers/hésitations) de l'oral qui polluent le texte écrit.

**Mécanisme** :
- 9 regex pré-compilées (une par langue) avec word-boundary `\b`
- Appliqué après les commandes dictée, avant le cleanup model
- Espaces multiples nettoyés via `RE_MULTI_SPACES`
- Toggle : `disfluency_removal_enabled` dans Preferences (défaut : activé)

**Fillers supprimés** :

| Langue | Fillers | Type |
|---|---|---|
| FR | euh, heu, hum, bah, ben, beh | Hésitation pure |
| EN | uh, um, hmm | Hésitation pure |
| DE | äh, ähm, hm, hmm, tja, naja | Hésitation pure |
| ES | eh, em, este, pues | Hésitation pure |
| PT | hum, tipo, né | Hésitation pure |
| IT | ehm, allora, cioè, ecco | Hésitation/discursif léger |
| NL | eh, ehm, uhm, nou | Hésitation pure |
| PL | yyy, eee, no, jakby | Hésitation pure |
| RU | э, эм, ну, вот, типа, как бы | Hésitation pure |

**Note** : les fillers discursifs (genre, du coup, like, you know) ne sont PAS supprimés car ils ont un sens réel dans certains contextes. Seuls les marqueurs d'hésitation purs sont ciblés.

**Latence** : ~0ms (regex pré-compilées)

### Alternatives ML écartées

| Approche | Précision | Latence | Raison d'exclusion |
|---|---|---|---|
| CTC Forced Alignment (disfluency detection) | 81.6% | ~100ms | Nécessite modèle CTC + alignement mot-par-mot |
| Smooth-LLaMa (LLM fine-tuné) | ~85% | ~500ms | LLM dédié trop lourd pour un gain marginal |
| Whisper word timestamps + durée | Variable | ~0ms (post-hoc) | Nécessite word-level timestamps, pas disponible sur tous les engines |

---

## Étape 4 — Ponctuation & Capitalisation (✅ Implémenté)

**Crates** :
- `crates/jona-engine-bert/` — catalogue + inférence BERT (ort ONNX + CoreML, ou Candle + Metal GPU selon le modèle). Logique partagée : `strip_and_split()`, `restore_punctuation_windowed()`, constantes `PUNCT_LABELS`, `WINDOW_SIZE` (230), `OVERLAP` (5)
- `crates/jona-engine-pcs/` — catalogue + inférence PCS (ONNX + SentencePiece protobuf → sliding window 128, overlap 16)

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

**Dispatch** : dynamique via `ASREngine::cleanup()` trait, routé dans `recording/pipeline.rs` via `EngineCatalog`.

---

## Étape 5 — Spell-check (✅ Implémenté)

**Fichier** : `cleanup/symspell_correct.rs` — fonction `symspell_correct()`

**But** : corriger les fautes d'orthographe évidentes avant la correction grammaticale.

**Mécanisme** :
- **SymSpell** (algorithme symmetric delete) avec dictionnaires téléchargeables depuis GitHub Releases
- Dictionnaires construits par `JonaWhisper/jonawhisper-spellcheck-dicts` (Ruby, CI mensuel)
- 6 variantes : fr, fr-be, fr-ca, fr-ch, en, en-gb
- Fréquences + bigrams par langue (FR : DELA 683K formes + Leipzig Corpora 100K bigrams)
- Chargement lazy depuis disque, cache par langue via `Mutex<HashMap>`
- `lookup_compound` pour les erreurs de frontières de mots (fréquentes en ASR)
- Résolution langue : essaie locale complète (fr-ca), puis langue de base (fr)
- **KenLM reranking** : les candidats SymSpell sont rescorés en contexte trigram via un modèle de langue KenLM (Wikipedia, pruned + quantized 8-bit). Le meilleur candidat en contexte est sélectionné. Dégradation gracieuse sans modèle LM.
- **French guards** : pluriel (mot finissant par 's' + radical connu → garder), apostrophe/élision (j'avais → "avais" connu → garder)
- **Dynamic max_distance** : edit distance 1 pour mots <6 chars, 2 pour les plus longs — réduit les faux positifs sur les mots courts
- **Chargement deadlock-free** : les dictionnaires (~645K mots FR) sont chargés HORS du mutex. Pattern : vérification rapide sous lock → chargement hors lock → insertion sous lock

**Toggle** : `spellcheck_enabled` dans Preferences (défaut : désactivé).

**Latence** : <1ms par phrase (SymSpell symmetric delete).

**RAM** : FR ~100 MB, EN ~30 MB (lazy-loaded à la première utilisation).

---

## Étape 6 — Correction grammaticale & orthographique (✅ Implémenté)

**Crate** : `crates/jona-engine-correction/` — catalogue (4 modèles T5) + inférence ONNX Runtime

### Architecture

```
Texte → tokenize (SentencePiece via tokenizer.json)
     → T5 Encoder (ONNX, ort + CoreML)
     → Décodage autorégressif (greedy, repeat penalty 1.5, n-gram blocking 4)
     → Texte corrigé
```

### Anti-hallucination

Le décodage T5 autorégressif peut halluciner (répétitions, divergence). Protections implémentées :

| Protection | Mécanisme | Paramètre |
|---|---|---|
| Repeat penalty | Pénalise les tokens déjà générés | 1.5 |
| N-gram blocking | Interdit les 4-grams déjà vus | Taille 4 |
| Détection de boucle live | Stop si les 6 derniers tokens forment un pattern déjà vu | Fenêtre 6 tokens |
| Longueur max génération | Limite le nombre de tokens générés | `input_tokens * 1.2 + 16` |
| Sanitisation sortie | Vide → garder original, >3x input → garder original | Automatique |
| Strip repetition post-hoc | Détecte les répétitions phrase/mot dans le texte final | Sentence + word level, seuil 80% |

### Modèles

| Modèle | Params | Taille | Langues | Spécialité | Vitesse |
|---|---|---|---|---|---|
| **GEC T5 Small** | 60M | 96 MB | 11 langues | GEC multilingue | ~200ms |
| T5 Spell FR | 220M | 276 MB | FR | Orthographe FR | ~500ms |
| FlanEC Base | 250M | 276 MB | EN | Post-ASR correction | ~500ms |
| FlanEC Large | 800M | 821 MB | EN | Post-ASR correction | ~1s |

**Dispatch** : dynamique via `ASREngine::cleanup()` trait, routé dans `recording/pipeline.rs` via `EngineCatalog`.

---

## Étape 7 — Finalize (✅ Implémenté)

**Fichier** : `cleanup/post_processor/mod.rs` — fonction `finalize()`

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

## Étape 8 — ITN (Inverse Text Normalization) (✅ Implémenté)

**Module** : `cleanup/itn/` — 9 fichiers par langue + `mod.rs` dispatch

**But** : convertir les nombres, dates, heures et devises de leur forme orale vers leur forme écrite canonique.

### Langues supportées

FR, EN, DE, ES, PT, IT, NL, PL, RU — chaque langue a son propre fichier avec atoms, multipliers, parser, regex rules et tests.

### Catégories couvertes (toutes langues)

| Catégorie | Exemple FR | Sortie | Exemple EN | Sortie |
|---|---|---|---|---|
| Nombres cardinaux | vingt-trois | 23 | twenty three | 23 |
| Nombres composés | quatre-vingt-dix-sept | 97 | two hundred and fifty | 250 |
| Milliers | trois mille deux cents | 3200 | three thousand two hundred | 3200 |
| Ordinaux | premier, deuxième | 1er, 2e | first, second | 1st, 2nd |
| Pourcentages | dix pour cent | 10 % | ten percent | 10 % |
| Heures | trois heures et quart | 3 h 15 | three o'clock | 3 :00 |
| Devises | cinq euros | 5 € | five dollars | 5 $ |
| Unités | deux kilomètres | 2 km | five miles | 5 mi |

### Architecture

```
cleanup/itn/
├── mod.rs    — dispatch apply_itn(), shared helpers (replace_numbers, apply_regex_list, regex_rules! macro)
├── fr.rs     — atoms (0-60, soixante-dix, quatre-vingts), composés hyphenés, "et" conjonctif
├── en.rs     — atoms 0-90, "and" conjonctif, composés hyphenés
├── de.rs     — composés inversés (dreiundzwanzig = 3+20), strip_suffix("hundert"/"tausend")
├── es.rs     — composés soudés (veintitrés), centaines genrées (doscientos/doscientas)
├── pt.rs     — composés "e" séparateur, centaines (duzentos, trezentos...)
├── it.rs     — élision voyelle (ventuno, trentotto), prefix matching pour décomposition
├── nl.rs     — composés inversés avec "en" (drieëntwintig), strip_suffix("honderd"/"duizend")
├── pl.rs     — centaines composées (dwieście, pięćset), milliers avec déclinaisons
└── ru.rs     — centaines soudées (двести-девятьсот), milliers avec déclinaisons (тысяча/тысячи/тысяч)
```

Chaque fichier par langue exporte `pub(super) fn apply_all(text: &str) -> String` qui chaîne : regex substitutions (%, heures, devises, ordinaux, unités) → parser de nombres cardinaux.

**Sécurité** :
- "un"/"une"/"a"/"один" isolés ne sont pas convertis (ambiguïté article/nombre) — sauf si suivis d'un mot d'unité (kilomètre, heure, etc.)
- "zéro"/"zero" est toujours converti (jamais un article)
- FR pourcentages : `pour cent` (2 mots) et `pourcent(s)` (1 mot) sont tous les deux reconnus

**Toggle** : `itn_enabled` dans Preferences (défaut : activé).

**Latence** : ~0ms (regex pré-compilées + lookup)

---

## Suivi des étapes du pipeline (✅ Implémenté)

Chaque étape du pipeline (ponctuation, spell-check, correction, ITN) enregistre son résultat dans `pipeline_steps` :

| Suffixe | Signification | Exemple |
|---|---|---|
| *(aucun)* | L'étape a modifié le texte | `("punctuation", "Texte ponctué.")` |
| `:nochange` | L'étape a tourné mais n'a rien changé | `("spellcheck:nochange", "")` |
| `:error` | L'étape a échoué | `("correction:error", "Model not loaded")` |

Le frontend affiche ces états dans le **pipeline stepper** (8 icônes dans `HistoryEntryCard.vue`) avec 4 états visuels :
- **Gris** : étape désactivée
- **Coloré** : étape active avec diff (cliquable → diff mot-à-mot inline)
- **Atténué + Slash** : étape active, aucun changement
- **Atténué + X** : étape en erreur

---

## Chaînage ponctuation + correction (✅ Implémenté)

Le pipeline dans `recording/pipeline.rs` (`handle_transcription_result()`) chaîne ponctuation et correction séquentiellement :

```
preprocess (hallucinations → dictée → disfluences)
  → ponctuation (PCS/BERT, via punctuation_model_id)
  → correction/LLM (via cleanup_model_id)
  → finalize (espacement, capitalisation)
  → ITN
  → paste
```

**Deux paramètres indépendants** :
- `punctuation_model_id` — choisit le modèle de ponctuation (PCS ou BERT)
- `cleanup_model_id` — choisit le modèle de correction (T5) ou LLM (local/cloud)

Le helper `run_local_engine()` factorise le dispatch (catalog lookup → spawn_blocking → contexts.run_with) pour les deux étapes.

**Migration v6** : déplace les modèles de ponctuation de `cleanup_model_id` vers `punctuation_model_id` pour les utilisateurs existants.

---

## Fichiers concernés

| Fichier | Rôle dans le pipeline |
|---|---|
| `recording/pipeline.rs` | Orchestration : `handle_transcription_result()` appelle les étapes dans l'ordre |
| `cleanup/post_processor/mod.rs` | Orchestration preprocess/finalize, ponctuation spacing, capitalisation |
| `cleanup/post_processor/hallucinations.rs` | Étape 1 : phrases connues (9 langues), musique, répétition |
| `cleanup/post_processor/dictation.rs` | Étape 2 : commandes dictée FR/EN |
| `cleanup/post_processor/fillers.rs` | Étape 3 : disfluences/fillers (9 langues) |
| `cleanup/itn/mod.rs` | Étape 8 : dispatch ITN + helpers partagés (replace_numbers, regex_rules!) |
| `cleanup/itn/{fr,en,de,es,pt,it,nl,pl,ru}.rs` | Étape 8 : ITN par langue (9 langues) |
| `cleanup/symspell_correct.rs` | Étape 5 : spell-check SymSpell (dicts téléchargeables, GitHub Releases) |
| `cleanup/llm_cloud.rs` | Cloud LLM cleanup (OpenAI/Anthropic API) |
| `cleanup/vad.rs` | VAD Silero v6.2 (ONNX, pré-transcription) |
| `crates/jona-engine-bert/` | Étape 4 : catalogue + inférence BERT punctuation (ort + Candle, 2 modèles) |
| `crates/jona-engine-pcs/` | Étape 4 : catalogue + inférence PCS punctuation (ort, 1 modèle, 47 langues) |
| `crates/jona-engine-correction/` | Étape 6 : catalogue + inférence T5 correction (ort ONNX, 4 modèles, décodage autorégressif) |
| `crates/jona-engine-llama/` | Étape 6 (alt.) : catalogue + inférence LLM local (llama-cpp-2, Metal GPU) |
| `crates/jona-engines/src/ort_session.rs` | Builder de session ort partagé (CoreML EP) — utilisé par BERT, PCS, T5 |
| `crates/jona-engines/src/lib.rs` | `EngineCatalog`, dispatch dynamique via `ASREngine` trait |
| `crates/jona-types/src/lib.rs` | Préférences : `punctuation_model_id`, `cleanup_model_id`, `text_cleanup_enabled`, `disfluency_removal_enabled`, `itn_enabled`, `spellcheck_enabled` |

---

## Recherche & décisions techniques

Cette section documente les recherches menées pour améliorer le pipeline texte : alternatives évaluées, décisions prises, et justifications.

### Spell-check : SymSpell + KenLM (choix retenu)

**Problème initial** : SymSpell seul dégradait la transcription — les corrections étaient basées uniquement sur la distance d'édition, sans contexte. Un mot rare mais correct pouvait être "corrigé" vers un mot fréquent inapproprié.

**Solution** : combiner SymSpell (génération de candidats rapide) avec KenLM (scoring contextuel trigram).

| Composant | Rôle | Source |
|---|---|---|
| **SymSpell** | Génère les candidats (symmetric delete, O(1) lookup) | `symspell` crate |
| **KenLM** | Score les candidats en contexte trigram (probabilité du mot sachant les 2 précédents) | C++ vendoré (`kenlm_ffi.cc`), LGPL 2.1+ |
| **Dictionnaires fréquence** | FR: DELA 683K formes + régionaux. EN: liste de fréquences | `jonawhisper-spellcheck-dicts` (GitHub Releases) |
| **Bigrams** | FR: 100K bigrams de Leipzig Corpora (3 corpus × 1M phrases) | Idem |
| **Modèles de langue** | 9 langues, Wikipedia, pruned trigram, quantized 8-bit, 50-100 MB/lang | `JonaWhisper/kenlm-models` (HuggingFace) |

**Pipeline** :
1. SymSpell `lookup` pour chaque mot inconnu → liste de candidats
2. KenLM score chaque candidat dans le contexte trigram de la phrase
3. Le candidat avec le meilleur score KenLM est retenu
4. Sans modèle KenLM installé → dégradation gracieuse, SymSpell seul

**Quantization des modèles KenLM** : les modèles bruts Wikipedia (~2-8 GB) sont prunés (trigram seulement) puis quantifiés 8-bit via KenLM `build_binary` avec `-q 8 -a 22 -b 8`. Résultat : 50-100 MB par langue au lieu de plusieurs GB.

**Alternatives évaluées et écartées** :

| Alternative | Raison d'exclusion |
|---|---|
| **GECToR** (encoder-based GEC) | Pas d'ONNX pré-exporté, EN-only, modèles non officiels (licence non-commerciale), effort d'intégration élevé |
| **Harper** (`harper-core`) | EN-only (v1.4.1), checks redondants avec `finalize()` + PCS |
| **GECToR multilingue** | Aucun modèle pré-entraîné multilingue disponible |
| **Truecasing NER** | PCS couvre déjà la capitalisation 47 langues, NER ajouterait complexité pour un gain marginal |
| **LanguageTool** | Java, lourd (500 MB+), latence élevée (server mode), licence LGPL mais complexité d'intégration |
| **rphonetic** (Soundex/Metaphone) | Évalué pour le filtrage phonétique des candidats SymSpell — reste une piste intéressante (les erreurs ASR sont phonétiquement proches), pas encore implémenté |

### T5 Correction : INT8 quantization

**Problème** : les modèles T5 FP32 sont trop lourds (220-800 MB) et lents pour de la correction post-ASR en temps réel.

**Solution** : quantization dynamique INT8 via `onnxruntime.quantization.quantize_dynamic`.

| Modèle | FP32 | INT8 | Réduction | Qualité |
|---|---|---|---|---|
| GEC T5 Small (60M) | 242 MB | 96 MB | -60% | Comparable |
| T5 Spell FR (220M) | 884 MB | 276 MB | -69% | Comparable |
| FlanEC Base (250M) | 955 MB | 276 MB | -71% | Comparable |
| FlanEC Large (800M) | 3.2 GB | 821 MB | -74% | Légère dégradation |

**Pipeline de conversion** (dans `jonawhisper-model-tools`) :
1. Source PyTorch (HuggingFace) → ONNX FP32 (`optimum.exporters.onnx.main_export`)
2. ONNX FP32 → ONNX INT8 (`onnxruntime.quantization.quantize_dynamic`)
3. Upload HuggingFace (`HfApi().upload_file()`)

Les modèles INT8 sont auto-préférés via `prefer_int8()` dans `jona-engine-correction/inference.rs` : si les fichiers INT8 existent, ils sont utilisés à la place des FP32.

### Dictionnaires SymSpell : architecture de build

**Repo** : `JonaWhisper/jonawhisper-spellcheck-dicts` (GitHub public)

**Architecture** :
- Script Ruby (`build_dicts.rb`) avec un fichier par langue (`langs/*.rb`)
- Chaque langue définit : `base_words`, `build_freq`, `build_bigrams`, `freq_separator`
- Variantes régionales (fr-be, fr-ca, fr-ch, en-gb) : héritent de la base + merge TSV (`data/*.tsv`)
- CI mensuel : build → compare manifest SHA256 → crée release versionnée si changement
- Distribution via GitHub Releases (pas de fichiers git-trackés)

**Ajouter une langue de base** :
1. `langs/xx.rb` avec les 4 méthodes
2. `spellcheck:xx` model dans `jona-engine-spellcheck`

**Ajouter une variante régionale** :
1. `data/xx-yy.tsv` (mot / fréquence / définition)
2. `langs/xx_yy.rb` (hérite base + merge TSV)
3. `spellcheck:xx-yy` model dans le crate

### KenLM : vendoring et intégration

**Choix du vendoring** : KenLM C++ est inclus directement dans `jona-engine-lm` (query-only sources, ~20 fichiers .cc/.hh). Licence LGPL 2.1+ compatible avec GPL-3.0 du projet.

**FFI** : wrapper minimal `kenlm_ffi.cc` exposant 7 fonctions C :
- `kenlm_load(path)` / `kenlm_free(model)` — cycle de vie
- `kenlm_score_word(model, word, state_in, state_out)` — score un mot en contexte
- `kenlm_score_sentence(model, sentence)` — score une phrase complète
- `kenlm_begin_state` / `kenlm_null_state` / `kenlm_vocab_index` — gestion d'état

**Rust wrapper** : `KenLMModel` (Send+Sync, mmap-backed, ~0 overhead de chargement).

**Modèles** : 9 langues (FR, EN, DE, ES, PT, IT, NL, PL, RU), entraînés sur Wikipedia, pruned trigram, quantized 8-bit. Hébergés sur HuggingFace `JonaWhisper/kenlm-models`. Pipeline de build dans `jonawhisper-model-tools` (GitHub Actions, matrix par langue).

### Ponctuation : PCS vs BERT

**PCS** a été retenu comme modèle recommandé :

| Critère | BERT (Fullstop) | PCS |
|---|---|---|
| Langues | 4-5 | **47** |
| Capitalisation | Non | **Oui** |
| Taille | 562 MB - 1.1 GB | **233 MB** |
| Vitesse | ~80-100ms | **~50ms** |
| Architecture | Token classification | **4 heads** (pre/post punct, caps, segmentation) |

BERT reste disponible comme alternative (certains utilisateurs le préfèrent pour des langues spécifiques).

### Pistes non encore implémentées

| Piste | Description | Effort | Impact |
|---|---|---|---|
| **Filtrage phonétique** | Score Soundex/Metaphone pour filtrer les faux positifs SymSpell | Modéré | Moyen |
| **Log-probs hallucinations** | Token log-probs + compression ratio Whisper | Modéré | Haut |
| **Passe unique LLM** | Un LLM local remplaçant spell+punct+GEC | Élevé | Haut |
| **Correction sélective** | Ne corriger que les segments à faible confiance ASR | Modéré | Moyen |
| **Ponctuation domain-adapted** | Fine-tuner sur corpus ASR oral | Élevé | Moyen |

---

## Roadmap

### Phase 1 — Quick wins ✅ Terminé

1. ~~**Regex disfluences** FR/EN dans `post_processor.rs`~~ — ✅ Implémenté (`strip_fillers()`)
2. ~~**Regex ITN** — nombres, ordinaux, %, heures, devises, unités FR/EN~~ — ✅ Implémenté (`cleanup/itn.rs` : parser compositionnel FR/EN complet)

### Phase 2 — Améliorations pipeline

3. ~~**Chaînage ponctuation + correction**~~ — ✅ Implémenté. Ponctuation et correction sont maintenant des paramètres indépendants avec exécution séquentielle.

### Phase 3 — Améliorations qualité (recherche)

9. **Filtrage hallucinations par log-probabilité** — Token log-probs + compression ratio pour détecter les hallucinations de manière plus robuste que les listes statiques. Papers : "Whispering LLaMA" (2023), "Hallucination detection in neural ASR" (2024). Haut impact, effort modéré.
10. ~~**Dictionnaire utilisateur / biasing contextuel**~~ — ✅ Implémenté. Panneau "Dictionnaire" avec mots protégés + mappings ITN (`pattern=replacement`).
11. **Filtrage phonétique SymSpell** — Score Soundex/Metaphone pour filtrer les faux positifs SymSpell. Les erreurs ASR sont phonétiquement proches → SymSpell ne le sait pas. Effort modéré.
12. **Passe unique LLM** — Un seul appel LLM local (Qwen3 4B) remplaçant spell+punct+GEC. Cohérence globale, moins de passes. Risque : latence, hallucinations LLM. Évaluer sur benchmark avant migration.
13. **Correction sélective par confiance** — Ne corriger que les segments à faible confiance ASR (Parakeet/Canary exposent des scores, Whisper non). Effort modéré.

### Évaluations terminées — items écartés

4. ~~**GECToR**~~ — ❌ Écarté (mars 2026). Pas d'ONNX pré-exporté, EN-only, modèles non officiels (licence non-commerciale), effort d'intégration élevé (export custom + décodeur de tags). Le T5 existant couvre déjà FR/EN avec une qualité correcte.
5. ~~**Harper** (`harper-core`)~~ — ❌ Écarté (mars 2026). EN-only confirmé (v1.4.1). Checks couverts (spelling, a/an, mots répétés, capitalisation) largement redondants avec `finalize()` et la ponctuation PCS. Pas de valeur ajoutée pour un outil bilingue FR/EN.
6. ~~**ITN parser FR avancé**~~ — ✅ Fait dans Phase 1 (parser compositionnel complet)
7. ~~**GECToR multilingue**~~ — ❌ Écarté. Aucun modèle multilingue pré-entraîné disponible.
8. ~~**Truecasing avancé (NER)**~~ — ❌ Écarté. PCS couvre déjà la capitalisation pour 47 langues. Les modèles NER ajouteraient complexité et latence pour un gain marginal sur les noms propres rares.

---

*Voir aussi : [BENCHMARK.md](BENCHMARK.md) pour les données de benchmark détaillées (WER, RTF, latences par modèle).*

*Dernière mise à jour : mars 2026*
