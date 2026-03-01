# Pipeline Prétraitement Audio

Architecture du pipeline audio entre la capture micro et la transcription Whisper. Trois niveaux de traitement, exécutés dans l'ordre.

## Vue d'ensemble

```
Micro (cpal) → [1. Calibration device] → [2. Denoising] → [3. VAD] → WAV → Whisper
                      │                        │                │
                 Gain, noise gate         Suppression       Silence ?
                 par type de micro        bruit de fond     → discard
```

## Niveau 1 — Calibration device (presets)

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

**Priorité** : Basse — c'est du polish UX, les deux niveaux suivants apportent plus de valeur immédiate

---

## Niveau 2 — Noise reduction (denoising)

**But** : supprimer le bruit de fond (ventilateur, clavier, rue, musique lointaine) tout en préservant la voix. Améliore directement la qualité de transcription Whisper.

**Quand** : soit en temps réel pendant l'enregistrement (sample par sample sur le stream cpal), soit en post-traitement sur le WAV juste avant Whisper.

**Options d'implémentation** (à confirmer après benchmark recherche en cours) :

| Solution | Type | Taille | Temps réel | VAD inclus | Rust | Note |
|---|---|---|---|---|---|---|
| **nnnoiseless** | Crate Rust | ~100 KB | Oui | Oui | Natif | Port Rust de RNNoise, zéro dépendance |
| **RNNoise** | C (FFI) | ~100 KB | Oui | Oui | Via FFI | Référence, très léger, Xiph.org |
| **DeepFilterNet** | ML (ONNX) | ~5 MB | Oui | Non | Via ort | Meilleure qualité, plus lourd |
| **Speex preprocessor** | C (FFI) | ~50 KB | Oui | Oui | Via FFI | Vieux mais éprouvé, inclut AGC |
| **Silero VAD + RNNoise** | Combo | ~2 MB | Oui | Oui (Silero) | Via ort + FFI | VAD précis + denoising |

**Approche recommandée** : commencer par **nnnoiseless** (pure Rust, zéro dépendance, inclut VAD). Si la qualité ne suffit pas, passer à DeepFilterNet (ONNX via ort, déjà utilisé pour BERT).

**Intégration dans le code** :
- `audio.rs` traite déjà les samples PCM frame par frame (callback cpal)
- Le denoising s'insère dans ce callback, avant l'écriture WAV
- Alternative : post-traitement du WAV dans `recording.rs` avant `transcriber::transcribe()`

---

## Niveau 3 — VAD / Détection de silence

**But** : détecter si l'enregistrement contient de la parole. Si non (appui accidentel, silence, bruit sans voix), discard directement sans envoyer à Whisper. Élimine les hallucinations sur audio vide.

**Quand** : après l'arrêt de l'enregistrement, avant la transcription.

**Approches** :

### Simple — Énergie RMS
- Calculer l'énergie RMS de l'audio
- Si sous un seuil → pas de parole → discard
- Rapide (~1ms), zéro dépendance
- Limité : ne distingue pas bruit fort vs parole

### Medium — VAD intégré à RNNoise/nnnoiseless
- RNNoise retourne une probabilité de voix par frame (0.0 à 1.0)
- Si la moyenne sur tout l'audio est sous un seuil → discard
- Gratuit si on utilise déjà nnnoiseless pour le denoising
- Bonne qualité, distingue bien bruit vs parole

### Avancé — Silero VAD (ONNX)
- Modèle ML dédié (~2 MB), très précis
- Timestamp-level : sait exactement où commence/finit la parole
- Pourrait aussi servir à couper les silences en début/fin d'enregistrement
- Via ort (déjà utilisé pour BERT)

**Approche recommandée** : utiliser le VAD de nnnoiseless (gratuit avec le denoising). Si on veut plus de précision ou du trimming de silence, ajouter Silero VAD.

**Intégration dans le code** :
- `recording.rs` dans `process_next_in_queue()`, entre la lecture du WAV et l'appel à `transcriber::transcribe()`
- Si pas de parole détectée : `platform::play_sound("Basso")` + return (même pattern que hallucination)

---

## Ordre d'implémentation suggéré

```
Phase 1 — Denoising + VAD (haute valeur, effort modéré)
├── Intégrer nnnoiseless dans audio.rs (denoising temps réel)
├── Utiliser le score VAD de nnnoiseless pour détecter le silence
├── Discard si pas de parole (son Basso + pas de transcription)
└── Setting on/off dans les préférences

Phase 2 — Amélioration qualité (si Phase 1 insuffisante)
├── Remplacer/compléter par DeepFilterNet (ONNX) pour meilleur denoising
├── Ajouter Silero VAD pour VAD plus précis + trimming silence
└── Trimmer les silences début/fin avant transcription

Phase 3 — Presets device (polish UX)
├── Détecter le type de micro (transport type déjà dispo)
├── Presets par défaut (built-in, USB, Bluetooth)
├── UI de configuration des presets
├── Gain, noise gate, normalisation par device
└── Sauvegarde des presets personnalisés
```

## Impact attendu

| Traitement | Réduit hallucinations | Améliore transcription | Effort |
|---|---|---|---|
| **VAD (silence detection)** | Fortement (cause racine) | Non | Faible |
| **Denoising** | Indirectement | Oui (audio bruité) | Modéré |
| **Presets device** | Indirectement | Oui (niveau/bruit) | Élevé |

---

## Fichiers concernés

| Fichier | Rôle dans le pipeline |
|---|---|
| `audio.rs` | Capture cpal, callback PCM → insertion denoising temps réel |
| `recording.rs` | Orchestration : stop → VAD check → transcribe → cleanup → paste |
| `platform/audio_devices.rs` | Détection device + transport type (pour presets) |
| `state.rs` | Préférences : denoising on/off, seuil VAD, presets device |

---

*Dernière mise à jour : mars 2026*
