# Recherche — Correction grammaticale multilingue

*Mars 2026 — Synthèse de la recherche pour améliorer/remplacer le T5 correction actuel.*

---

## Contexte

Le T5 correction actuel (Candle, autorégressif) a des problèmes de stabilité (boucles, crashes Metal). L'objectif est de trouver des alternatives plus fiables, rapides, et multilingues (FR + EN minimum).

---

## 1. LanguageTool

**Qu'est-ce que c'est** : correcteur open source rule-based, 31 langues (dont FR/EN), LGPL-2.1.

| Option | Avantages | Inconvénients |
|--------|-----------|---------------|
| **API publique** | 31 langues, 6000+ règles EN | Rate limit 20 req/min, latence 50-3000ms, nécessite internet |
| **Serveur local (Java)** | Offline, toutes les règles | JVM requis, 500MB+ RAM, memory leaks reportés |
| **languagetool-rust** (crate) | Client HTTP Rust, async | Juste un wrapper HTTP — nécessite un serveur |

**nlprule** (crate Rust) : port natif des règles LanguageTool. Pur Rust, embarqué, rapide. **Mais** : EN/DE/ES seulement, **pas de FR**, dernière release avril 2021 (abandonné).

**Verdict** : LanguageTool est excellent en qualité mais incompatible avec notre architecture (pas de JVM, pas de serveur externe). nlprule serait parfait mais pas de FR et abandonné. ❌

---

## 2. Modèles HuggingFace multilingues

### Modèles les plus pertinents

| Modèle | Base | Params | Taille | Langues | F₀.₅ | Format |
|--------|------|--------|--------|---------|-------|--------|
| **Unbabel/gec-t5_small** | T5-small | 60M | 242 MB (fp32) / 121 MB (fp16) | Multi (cLang-8) | 60.70 | safetensors |
| **flexudy/t5-base-multi-sentence-doctor** | T5-base | 220M | ~260 MB | EN, DE, **FR** | — | safetensors |
| **grammarly/coedit-large** | Flan-T5-Large | 770M | ~1.5 GB | Multi | — | safetensors |
| **gotutiyan/gector-deberta-large-5k** | DeBERTa-large | 400M | ~800 MB | EN only | 65.3 | safetensors |

### Modèles SOTA (trop gros pour on-device)

| Modèle | Params | Notes |
|--------|--------|-------|
| Gemma-2 + LoRA (OmniGEC) | 9B | Vainqueur MultiGEC-2025, 11 langues |
| Aya-Expanse-8B (OmniGEC) | 8B | Fine-tuné sur 11 langues |

---

## 3. Approches par catégorie

### Sequence tagging (non-autorégressif, rapide)

| Approche | Vitesse | Précision | Langues | ONNX | Statut |
|----------|---------|-----------|---------|------|--------|
| **GECToR** | 10x T5 | F₀.₅ 65.3 | EN only | Export possible | Pas de modèle ONNX prêt |
| **PIE** | 5-15x T5 | — | EN only | TensorFlow | Recherche uniquement |
| **LM-Critic** | Lent (LM scoring) | — | EN only | — | Recherche |

**Verdict** : rapide mais EN-only, pas d'ONNX prêt, effort d'intégration élevé.

### Seq2seq (autorégressif, plus lent mais plus flexible)

| Approche | Vitesse | Avantage | Inconvénient |
|----------|---------|----------|--------------|
| T5-small/base | 50-100ms | Multilingue, ONNX exportable | Autorégressif, hallucinations possibles |
| mBART | ~100ms | Bon pour multilingue | Plus lourd que T5 |
| mEdIT (CoEdIT multilingue) | 50-150ms | 7 langues, instruction-tuned | Overkill pour GEC seul |

### Hybride (règles + ML)

| Approche | Description |
|----------|-------------|
| **Spell-check + T5** | zspell (20 MB) détecte les fautes → T5-small corrige les segments flaggés |
| **LanguageTool + neural reranking** | Règles détectent → réseau léger classe | Nécessite JVM |

---

## 4. Correction post-ASR spécifiquement

Recherche récente ciblant exactement notre cas (ASR → correction) :

| Approche | Résultat | Paper |
|----------|----------|-------|
| **DARAG** | 8-30% WER improvement | arXiv:2410.13198 |
| **Conformer multi-candidat** | 21% WER reduction | arXiv:2409.09554 |
| **Flan-T5 fine-tuné ASR** | WER 13.1% → 4.2% ("Whispering LLaMA") | EMNLP 2023 |
| **GenSEC Challenge** (NVIDIA) | Benchmark post-ASR | 2024 |

**Insight clé** : les modèles GEC génériques ne comprennent pas les erreurs typiques ASR (homophones, mots coupés, segments manquants). Un fine-tuning sur des paires (ASR brut → texte correct) donne des résultats bien supérieurs.

---

## 5. Spell-checking Rust

| Crate | Type | FR | Taille dict | Notes |
|-------|------|----|----|-------|
| **zspell** 0.5.5 | Pur Rust, Hunspell-compatible | ✅ (OpenOffice dicts) | ~20 MB en mémoire | Recommandé |
| **hunspell-rs** 0.4.0 | FFI Hunspell | ✅ | Variable | Dépendance C |
| **symspell** | Symmetric delete | ✅ (dict custom) | Variable | Fuzzy matching |

---

## 6. Recommandations pour JonaWhisper

### Court terme — Remplacement T5

**Option recommandée : `Unbabel/gec-t5_small`** (ou notre GEC T5 Small existant, qui est le même modèle)
- 60M params, 242 MB, multilingue
- On l'a déjà dans le catalogue (`correction:gec-t5-small`)
- Le problème n'est pas le modèle mais l'**infrastructure d'inférence** (Candle autorégressif + Metal → crashes)
- **Alternative** : exporter en ONNX et inférer via `ort` (comme PCS/BERT) — élimine les problèmes Metal/Candle

### Moyen terme — Spell-check rule-based

Ajouter `zspell` comme couche rapide (~5ms) :
1. Détecte les fautes d'orthographe courantes (homophones ASR)
2. Auto-corrige les cas évidents (edit distance 1, candidat unique)
3. Pas besoin de modèle ML pour 80% des erreurs ASR simples
4. FR + EN via dictionnaires OpenOffice

### Long terme — Fine-tuning post-ASR

Fine-tuner un T5-small sur des paires (transcription ASR → texte corrigé) pour notre cas d'usage spécifique. Les résultats de la recherche montrent des gains de 20-70% WER avec cette approche.

---

## Références clés

- [A Simple Recipe for Multilingual GEC](https://arxiv.org/abs/2106.03830) — Unbabel, 2021
- [OmniGEC Dataset](https://arxiv.org/abs/2509.14504) — 11 langues, 2025
- [DARAG: Post-ASR Correction](https://arxiv.org/abs/2410.13198) — 2024
- [GenSEC Challenge](https://research.nvidia.com/publication/2024-12_large-language-model-based-generative-error-correction-challenge-and-baselines) — NVIDIA
- [MultiGEC-2025 Shared Task](https://spraakbanken.github.io/multigec-2025/)
- [GECToR](https://arxiv.org/abs/2005.12592) — Grammarly
- [CoEdIT / mEdIT](https://arxiv.org/abs/2305.09857) — Google
- [zspell](https://github.com/pluots/zspell) — Pure Rust spell checker
- [nlprule](https://github.com/bminixhofer/nlprule) — Rust LanguageTool port (abandonné)

---

*Dernière mise à jour : mars 2026*
