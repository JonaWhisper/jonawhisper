# Pipeline Prétraitement Audio

Architecture du pipeline audio entre la capture micro et la transcription Whisper.

## Constat critique : le denoising dégrade Whisper

Le paper **"When De-noising Hurts"** (arXiv:2512.17562, déc 2025) a évalué systématiquement le speech enhancement sur 4 systèmes ASR (Whisper, Parakeet, Gemini, Parrotlet) sous 9 conditions de bruit :

- **L'audio original brut obtient un meilleur WER que l'audio nettoyé dans 40/40 configurations testées**
- Dégradation de 1.1% à 46.6% de WER absolu
- Même en conditions propres, le denoising pénalise (1.32% à 3.19%)
- Les artefacts spectraux du denoising (smearing, discontinuités temporelles) sont **plus nuisibles** que le bruit original

**Pourquoi** : Whisper est entraîné sur 680K heures d'audio diversifié incluant du bruit réel. Il a appris une robustesse interne au bruit. Le denoising modifie les représentations spectrales d'une manière que Whisper n'a jamais vue.

**Exception** : le denoising aide si l'ASR est **fine-tuné sur de l'audio dénoisé** (pas notre cas avec Whisper standard).

**Conséquence** : le denoising envoyé directement à l'ASR est nuisible. Cependant, le paper ne teste pas l'utilisation du dénoisé pour **améliorer la VAD** (boundaries) ou le **playback** (confort d'écoute). L'approche hybride (dénoisé pour VAD/playback, original pour ASR) contourne le problème.

La priorité absolue reste le **VAD** (trimming silence → prévient les hallucinations).

Sources : [arXiv:2512.17562](https://arxiv.org/abs/2512.17562), [Whisper discussion #2125](https://github.com/openai/whisper/discussions/2125)

---

## Vue d'ensemble (révisée)

```
Pipeline actuel :
  Micro (cpal) → WAV 16 kHz → [1. VAD (Silero v5)] → Trim → ASR

Pipeline hybride proposé :
  Micro (cpal) → WAV 16 kHz ──┬──► [Denoise copie] → VAD (meilleurs boundaries)
                               │                           │ trim
                               └──► ASR sur ORIGINAL ◄─────┘
                               └──► [3. Calibration device] → Gain, noise gate
```

**Changement de priorité** : VAD en premier (haute valeur, cause racine des hallucinations), denoising optionnel (peut dégrader Whisper), calibration en dernier (polish UX).

---

## Niveau 1 — VAD / Détection de silence (PRIORITÉ HAUTE)

**But** : détecter si l'enregistrement contient de la parole. Si non (appui accidentel, silence, bruit sans voix), discard directement sans envoyer à Whisper. Élimine les hallucinations sur audio vide.

**Quand** : après l'arrêt de l'enregistrement, avant la transcription.

### Options d'implémentation

| Solution | Taille | Latence | Précision (TPR @ 5% FPR) | Rust | Dépendances |
|---|---|---|---|---|---|
| **Silero VAD v5** | 2.3 MB | <1ms / 30ms chunk | ROC-AUC : AliMeeting 0.96, AISHELL-4 0.94 | Direct `ort` (pas de crate VAD — conflits ndarray) | `ort` (ONNX Runtime) |
| **Silero VAD v6.2** | 2 MB | <1ms / 30ms chunk | +16% vs v5 sur bruit réel | Direct `ort` (drop-in) | `ort` |
| **Earshot** | **75 KB** | <0.1ms / chunk | Non publié (base WebRTC NN) | `earshot` crate (Rust pur, no_std) | Aucune |
| **TEN VAD** | 2.2 MB | Ultra-faible | Non publié, claims meilleures transitions | ONNX via `ort` | `ort` — **⚠ clause non-compete** |
| RNNoise/nnnoiseless VAD | ~85 KB | <1ms / 10ms frame | Correct (~70%) | Natif (inclus dans nnnoiseless) | Aucune |
| WebRTC VAD | 158 KB | ~0ms | **50% TPR** @ 5% FPR | `webrtc-vad` crate (abandonné 2019) | C FFI |
| Énergie RMS | 0 | ~1ms | Limité (pas voix vs bruit) | Natif | Aucune |
| NVIDIA MarbleNet v2 | ~400 KB | <1ms / 20ms | Bon | ONNX via `ort` | `ort` — licence NVIDIA OML restrictive |

**Recommandation** : **Silero VAD v5** — ✅ **Implémenté** via `ort` directement (pas de crate VAD dédiée : `silero-vad-rust`, `silero-vad-rs`, `voice_activity_detector` ont tous des conflits ndarray avec ort 2.0.0-rc.11). Modèle ONNX (~2.3 MB) embarqué via `include_bytes!`. Voir `src-tauri/src/vad.rs`.
- 4x moins d'erreurs que WebRTC VAD
- <1ms par chunk, 2.3 MB de modèle, état LSTM [2,1,128] + contexte 64 samples
- Inférence directe : `forward_chunk` → probabilité 0.0-1.0
- L'app utilise déjà `ort` pour BERT/PCS punctuation + ASR → pas de nouvelle dépendance

**Upgrade ciblé** : Silero v6.2 est un drop-in (+16% précision sur bruit réel, voix enfants/étouffées). CoreML pré-converti disponible.

**Alternative légère** : `earshot` (pyke.io, même équipe que `ort`) — 75 KB, Rust pur, no_std, ~20x plus rapide. À évaluer sur audio réel JonaWhisper.

**Fonctionnalités VAD** :
1. **Discard silence** : si aucun segment de parole détecté → son Basso + pas de transcription
2. **Trimming** : couper les silences début/fin avant envoi à Whisper → réduit le temps de transcription + prévient hallucinations sur silence trailing

**Intégration dans le code** :
- `recording.rs` dans `process_next_in_queue()`, entre la lecture du WAV et l'appel à `transcriber::transcribe()`
- Si pas de parole détectée : `platform::play_sound("Basso")` + return (même pattern que hallucination LLM)

---

## Niveau 2 — Noise reduction (OPTIONNEL, désactivé par défaut)

**But** : supprimer le bruit de fond. Peut améliorer le confort d'écoute (playback historique) et la précision VAD, mais **dégrade la transcription ASR si envoyé directement**.

**Approche hybride recommandée** : ne jamais envoyer l'audio dénoisé directement à l'ASR. À la place :
```
Original 16 kHz WAV ──┬──► [Denoise copie] ──► VAD (boundaries précises)
                      │                              │
                      │                    trim start/end
                      │                              │
                      └──► ASR sur ORIGINAL (trimmed) ◄──┘
                      └──► [Copie dénoisée pour playback historique]
```

Le paper "When De-noising Hurts" (arXiv:2512.17562) teste uniquement le denoising → ASR directement. Il ne couvre pas l'utilisation du dénoisé pour améliorer la VAD ni le playback. L'approche hybride contourne le problème : l'ASR reçoit toujours l'audio original.

**Usage recommandé** : toggle optionnel dans les préférences, désactivé par défaut. Utile pour :
- **Pipeline hybride** : améliorer les boundaries VAD en environnement bruité
- Nettoyer l'audio sauvegardé dans l'historique (confort d'écoute)
- Environnements extrêmement bruités (SNR < 5 dB) où l'utilisateur préfère tenter

### Options d'implémentation

#### nnnoiseless — Pure Rust, le plus simple

- **Crate** : `nnnoiseless` 0.5.2 (déc 2025, activement maintenu)
- **Type** : Port Rust de RNNoise (GRU recurrent neural network)
- **Taille** : ~85 KB poids embarqués (quantifiés 8-bit)
- **Qualité** : PESQ ~3.88, STOI ~0.92
- **Latence** : <1ms par frame (480 samples @ 48 kHz = 10ms)
- **VAD inclus** : probabilité de voix par frame (bonus gratuit)
- **API** : `DenoiseState::new()` → `process_frame(output, input)` avec `[f32; 480]`
- **Valeurs audio** : plage [-32768.0, 32767.0] (échelle 16-bit, pas normalisé)
- **Sample rate** : fixe 48 kHz → resampling nécessaire si cpal fournit un autre taux
- **Thread-safe** : `Send + Sync + Clone`
- **Note** : première frame à ignorer (artefact fade-in)

#### DeepFilterNet3 — Meilleure qualité, natif Rust

- **Crate** : `deep_filter` (pipeline Rust natif via `tract` inference engine)
- **Taille** : ~8 MB modèle
- **Qualité** : PESQ 3.5+, STOI >0.95 — la meilleure qualité parmi les options
- **Latence** : ~20ms algorithmique
- **RTF** : 0.04 sur i5, encore plus rapide sur Apple Silicon
- **Sample rate** : 48 kHz full-band
- **Pas de VAD** intégré
- **ONNX export** : 3 fichiers (enc.onnx, erb_dec.onnx, df_dec.onnx) pour `ort` si besoin
- **Note** : crate crates.io un peu stale (2022), utiliser le repo git directement

#### DTLN — Alternative légère avec Rust existant

- **Crate** : `dtln-rs` (par Datadog, production-quality, Rust + WASM)
- **Taille** : ~2 MB (2 modèles ONNX)
- **Latence** : 32ms bloc, 8ms shift
- **Performance** : 33ms/s sur M1 MacBook
- **Sample rate** : 16 kHz (pas de resampling nécessaire pour Whisper qui est aussi 16 kHz)

#### Modèles ultra-légers (hidden gems)

| Modèle | Params | Taille ONNX | PESQ | STOI | DNSMOS | Sample Rate | Note |
|---|---|---|---|---|---|---|---|
| **GTCRN** | **48.2K** | <100 KB | 2.87 | 0.940 | 3.44 | **16 kHz** | Ultra-léger, ICASSP 2024, streaming natif |
| **UL-UNAS** | **169K** | ~500 KB | 3.09 | — | — | **16 kHz** | Évolution de GTCRN, bien meilleure qualité (mars 2025) |

Ces modèles sont en **16 kHz natif** (pas de resampling nécessaire — notre pipeline est déjà en 16 kHz). Nécessitent ONNX export + `ort` crate + CoreML + gestion STFT/ISTFT manuelle.

**Avantage clé vs nnnoiseless** : pas besoin de `rubato` pour le resampling 48→16 kHz.

**Recommandation par priorité** :
- **16 kHz natif préféré** : GTCRN (<100 KB, ort + CoreML) ou UL-UNAS (~500 KB, meilleur PESQ) — pas de resampling
- **Qualité max** : nnnoiseless (PESQ 3.88, Rust pur, mais 48 kHz → `rubato` resampling requis)
- **Meilleur équilibre** : UL-UNAS (16 kHz, PESQ 3.09, MIT, ort, CoreML possible)

---

## Niveau 3 — Calibration device (presets)

**But** : normaliser le signal d'entrée selon le micro utilisé. Un AirPod, un micro USB studio et le micro intégré MacBook n'ont pas le même niveau, le même bruit de fond, ni la même réponse.

**Traitements** :
- **Gain** — amplification/atténuation pour atteindre un niveau cible
- **Noise gate** — couper le signal sous un seuil (élimine le bruit de fond constant)
- **Normalisation** — ramener le niveau de sortie à une plage cohérente

**Détection device** :
- L'app connaît déjà le device sélectionné (`audio_devices.rs` détecte le type de transport : built-in, USB, Bluetooth, etc.)
- Matcher par pattern dans le nom ("AirPods" → preset Bluetooth, "MacBook" → preset intégré)
- Presets par défaut + possibilité de personnaliser

**Dépendances** : aucune lib externe, c'est du traitement PCM basique (multiplication gain, threshold gate)

**Note Apple** : `AUVoiceProcessingIO` (AudioUnit) offre noise suppression + AGC + echo cancellation intégrés, mais est conçu pour la VoIP bidirectionnelle, pas pour l'enregistrement unidirectionnel. `AVCaptureDevice.MicrophoneMode.voiceIsolation` n'a pas d'API programmatique (préférence système utilisateur uniquement).

**Priorité** : Basse — c'est du polish UX

---

## Ordre d'implémentation (révisé)

```
Phase 1 — VAD seul (haute valeur, effort faible) ✅ DONE
├── Silero VAD v5 ONNX embarqué, inférence via ort (pas de crate VAD — conflits ndarray)
├── Discard si pas de parole (son Basso + pas de transcription)
├── Trimming silences début/fin avant Whisper
└── Toggle vad_enabled dans Settings > Post-traitement (activé par défaut)

Phase 1.5 — Améliorations VAD (haute valeur, effort faible)
├── Upgrade Silero v5 → v6.2 (swap .onnx, même API, CoreML dispo)
└── Évaluer earshot (75 KB, Rust pur, 20x plus rapide) — A/B test sur audio réel

Phase 2 — Pipeline hybride denoising (valeur conditionnelle, effort modéré)
├── Intégrer GTCRN ou UL-UNAS via ort (16 kHz natif, <500 KB)
├── Pipeline hybride : dénoisé pour VAD boundaries, original pour ASR
├── Toggle dans les préférences (désactivé par défaut)
├── Copie dénoisée stockée pour playback historique
├── Alternative : nnnoiseless (Rust pur, PESQ 3.88, mais 48 kHz → rubato)
└── Ne JAMAIS envoyer le dénoisé directement à l'ASR (arXiv:2512.17562)

Phase 3 — Presets device (polish UX)
├── Détecter le type de micro (transport type déjà dispo)
├── Presets par défaut (built-in, USB, Bluetooth)
├── UI de configuration des presets
├── Gain, noise gate, normalisation par device
└── Sauvegarde des presets personnalisés
```

## Impact attendu (révisé)

| Traitement | Réduit hallucinations | Améliore transcription | Risque | Effort |
|---|---|---|---|---|
| **VAD (Silero)** | **Fortement** (cause racine) | Oui (trimming → moins de bruit) | Aucun | Faible |
| **Denoising** | Indirectement (via meilleure VAD) | **Non directement** — dégrade ASR si envoyé tel quel, mais aide la VAD en pipeline hybride | WER +1-47% si direct | Modéré |
| **Presets device** | Indirectement | Oui (niveau/bruit) | Aucun | Élevé |

---

## Fichiers concernés

| Fichier | Rôle dans le pipeline |
|---|---|
| `recording.rs` | Orchestration : stop → **VAD check** → transcribe → cleanup → paste |
| `audio.rs` | Capture cpal, callback PCM → insertion denoising temps réel (si activé) |
| `platform/audio_devices.rs` | Détection device + transport type (pour presets) |
| `state.rs` | Préférences : VAD on/off, denoising on/off, seuil VAD, presets device |

---

## Dépendances Rust recommandées

| Crate | Version | Usage | Taille |
|---|---|---|---|
| `ort` | 2.0.0-rc.11 | ONNX Runtime — Silero VAD + BERT punctuation | Partagé |
| `ndarray` | 0.17 | Tensors pour VAD (état LSTM) | Partagé |
| `nnnoiseless` | 0.5.2 | Denoising optionnel | ~85 KB embarqué |
| `deep_filter` | Git | Denoising haute qualité (Phase 2+) | ~8 MB modèle |

---

*Dernière mise à jour : mars 2026*
