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

**Conséquence** : le denoising doit être **optionnel et désactivé par défaut**. La priorité absolue est le **VAD** (trimming silence → prévient les hallucinations).

Sources : [arXiv:2512.17562](https://arxiv.org/abs/2512.17562), [Whisper discussion #2125](https://github.com/openai/whisper/discussions/2125)

---

## Vue d'ensemble (révisée)

```
Micro (cpal) → [1. VAD] → [2. Denoising optionnel] → [3. Calibration device] → WAV → Whisper
                  │              │                           │
             Silence ?      Suppression bruit          Gain, noise gate
             → discard      (désactivé par défaut)     par type de micro
```

**Changement de priorité** : VAD en premier (haute valeur, cause racine des hallucinations), denoising optionnel (peut dégrader Whisper), calibration en dernier (polish UX).

---

## Niveau 1 — VAD / Détection de silence (PRIORITÉ HAUTE)

**But** : détecter si l'enregistrement contient de la parole. Si non (appui accidentel, silence, bruit sans voix), discard directement sans envoyer à Whisper. Élimine les hallucinations sur audio vide.

**Quand** : après l'arrêt de l'enregistrement, avant la transcription.

### Options d'implémentation

| Solution | Taille | Latence | Précision (TPR @ 5% FPR) | Rust | Dépendances |
|---|---|---|---|---|---|
| **Silero VAD v6** | 2 MB | <1ms / 30ms chunk | **87.7%** | `voice_activity_detector` ou `silero-vad-rust` | `ort` (ONNX Runtime) |
| **TEN VAD** | ~1 MB | Ultra-faible | Meilleur sur transitions | ONNX via `ort` | `ort` |
| RNNoise/nnnoiseless VAD | ~85 KB | <1ms / 10ms frame | Correct (~70%) | Natif (inclus dans nnnoiseless) | Aucune |
| WebRTC VAD | ~10 KB | ~0ms | **50%** (2x moins que Silero) | `webrtc-vad` crate | C FFI |
| Énergie RMS | 0 | ~1ms | Limité (pas voix vs bruit) | Natif | Aucune |
| NVIDIA MarbleNet v2 | ~400 KB | <1ms / 20ms | Bon | ONNX via `ort` | `ort` |

**Recommandation** : **Silero VAD v6** via crate `voice_activity_detector` ou `silero-vad-rust`.
- 4x moins d'erreurs que WebRTC VAD
- <1ms par chunk, 2 MB de modèle
- API simple : `forward_chunk(audio)` → probabilité 0.0-1.0
- `get_speech_timestamps()` pour le trimming
- Core ML disponible (`FluidInference/silero-vad-coreml`) si on veut Apple Neural Engine
- L'app utilise déjà `ort` pour BERT punctuation → pas de nouvelle dépendance

**Fonctionnalités VAD** :
1. **Discard silence** : si aucun segment de parole détecté → son Basso + pas de transcription
2. **Trimming** : couper les silences début/fin avant envoi à Whisper → réduit le temps de transcription + prévient hallucinations sur silence trailing

**Intégration dans le code** :
- `recording.rs` dans `process_next_in_queue()`, entre la lecture du WAV et l'appel à `transcriber::transcribe()`
- Si pas de parole détectée : `platform::play_sound("Basso")` + return (même pattern que hallucination LLM)

---

## Niveau 2 — Noise reduction (OPTIONNEL, désactivé par défaut)

**But** : supprimer le bruit de fond. Peut améliorer le confort d'écoute (pour le playback dans l'historique) mais **dégrade la qualité de transcription Whisper**.

**Usage recommandé** : toggle optionnel dans les préférences, désactivé par défaut. Utile uniquement pour :
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

| Modèle | Params | Taille ONNX | PESQ | Note |
|---|---|---|---|---|
| **GTCRN** | **23.7K** | <100 KB | 2.87 | Le plus petit existant (ICASSP 2024) |
| **UL-UNAS** | **169K** | ~500 KB | 3.09 | Évolution de GTCRN, bien meilleure qualité (mars 2025) |

Ces modèles nécessitent ONNX export + `ort` crate + gestion STFT/ISTFT manuelle.

**Recommandation** : si denoising activé → **nnnoiseless** (le plus simple à intégrer, pure Rust, inclut VAD bonus). Si qualité insuffisante → **DeepFilterNet3** (meilleure qualité, déjà en Rust via tract).

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
Phase 1 — VAD seul (haute valeur, effort faible)
├── Intégrer Silero VAD v6 via crate voice_activity_detector
├── Discard si pas de parole (son Basso + pas de transcription)
├── Trimming silences début/fin avant Whisper
└── Aucun impact négatif sur la qualité Whisper

Phase 2 — Denoising optionnel (valeur conditionnelle, effort modéré)
├── Intégrer nnnoiseless (pure Rust, ~85 KB)
├── Toggle dans les préférences (désactivé par défaut)
├── Avertir l'utilisateur que ça peut dégrader la transcription
├── Utile pour : playback audio propre dans l'historique
└── Si qualité insuffisante : DeepFilterNet3 via deep_filter crate

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
| **Denoising** | Indirectement | **Non — peut dégrader** | WER +1-47% | Modéré |
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
| `voice_activity_detector` ou `silero-vad-rust` | Latest | Silero VAD v6 | ~2 MB modèle |
| `ort` | 2.x | ONNX Runtime (déjà utilisé pour BERT) | Partagé |
| `nnnoiseless` | 0.5.2 | Denoising optionnel | ~85 KB embarqué |
| `deep_filter` | Git | Denoising haute qualité (Phase 2+) | ~8 MB modèle |

---

*Dernière mise à jour : mars 2026*
