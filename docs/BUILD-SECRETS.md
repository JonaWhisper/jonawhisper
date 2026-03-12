# Build Secrets & Environment Variables

Toutes les variables sont gérées par la CI (GitHub Actions).
Les builds locaux (`build.sh`) n'ont besoin d'aucun secret — le Keychain macOS gère la signature automatiquement.

## Secrets GitHub Actions

### macOS — Signature & Notarisation

| Secret | Description | Comment l'obtenir |
|--------|-------------|-------------------|
| `APPLE_CERTIFICATE` | Developer ID (.p12) en base64 | Exporter depuis Keychain Access, puis `base64 -i cert.p12 \| pbcopy` |
| `APPLE_CERTIFICATE_PASSWORD` | Mot de passe du .p12 | Choisi lors de l'export |
| `APPLE_SIGNING_IDENTITY` | Ex: `Developer ID Application: Nom (TEAM_ID)` | `security find-identity -v -p codesigning` |
| `APPLE_ID` | Email du compte Apple Developer | developer.apple.com |
| `APPLE_PASSWORD` | App-specific password | appleid.apple.com > Mots de passe pour les apps |
| `APPLE_TEAM_ID` | Team ID (10 caractères) | developer.apple.com > Membership |

**Prérequis :** Compte Apple Developer Program (99$/an) + certificat "Developer ID Application"

### Windows — Signature via Certum Cloud HSM

Le certificat EV Code Signing est stocké dans le HSM cloud de Certum.
La signature se fait via SimplySign (émule une carte à puce) + `signtool.exe` sur le runner Windows.

#### Achat et activation

1. Acheter "EV Code Signing in the Cloud" sur [shop.certum.eu](https://shop.certum.eu/ev-code-signing-in-the-cloud.html) (~226 EUR/an)
2. Activer le certificat dans Mon compte > Data security products > Activate
3. Installer SimplySign Desktop sur une machine Windows
4. Se connecter et scanner le QR code (contient le seed TOTP pour l'automatisation CI)

#### Secrets GitHub

| Secret | Description | Comment l'obtenir |
|--------|-------------|-------------------|
| `CERTUM_OTP_URI` | URI otpauth extraite du QR code SimplySign | Scanner le QR code affiché lors de l'activation |
| `CERTUM_USER_ID` | Identifiant du compte Certum | Email ou ID numérique |
| `CERTUM_PASSWORD` | Mot de passe du compte Certum | Choisi à l'inscription |

#### Comment ça marche en CI

1. Le runner Windows installe SimplySign Desktop
2. Un script PowerShell génère le code TOTP à partir de `CERTUM_OTP_URI`
3. SimplySign s'active et émule une carte à puce
4. `signtool.exe` signe l'exe via la carte à puce virtuelle
5. Le timestamp server `https://timestamp.certum.pl` horodate la signature

#### Config Tauri (`tauri.conf.json`)

La config actuelle utilise `signtool.exe` qui détecte automatiquement le certificat
via SimplySign. Pas besoin de `signCommand` — Tauri appelle signtool nativement
quand `certificateThumbprint` est configuré :

```json
{
  "bundle": {
    "windows": {
      "certificateThumbprint": "THUMBPRINT_DU_CERT_CERTUM",
      "digestAlgorithm": "sha256",
      "timestampUrl": "https://timestamp.certum.pl"
    }
  }
}
```

Le thumbprint est visible dans SimplySign Desktop ou via PowerShell :
`Get-ChildItem Cert:\CurrentUser\My | Format-Table Thumbprint, Subject`

#### Références

- [Defguard: Tauri + Certum HSM](https://defguard.net/blog/windows-codesign-certum-hsm/)
- [Automatiser SimplySign en CI](https://www.devas.life/how-to-automate-signing-your-windows-app-with-certum/)
- [certum-container (Linux CI)](https://github.com/hpvb/certum-container)

### Auto-Updater — Signature des mises à jour

| Secret | Description | Comment l'obtenir |
|--------|-------------|-------------------|
| `TAURI_SIGNING_PRIVATE_KEY` | Contenu de `~/.tauri/jona-whisper.key` | `npx tauri signer generate -w ~/.tauri/jona-whisper.key` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Mot de passe de la clé (vide actuellement) | Choisi à la génération |

La clé publique est embarquée dans `tauri.conf.json` > `plugins.updater.pubkey`.

**IMPORTANT :** Ne jamais perdre la clé privée — sans elle, impossible de publier des mises à jour pour les installations existantes. Backup : `~/.tauri/jona-whisper.key`

### Canaux de mise à jour

Deux canaux de mise à jour sont générés par la CI :

| Fichier | Canal | Contenu |
|---------|-------|---------|
| `latest-stable.json` | Stable | Uniquement les builds signés (codesign Apple / EV Certum) |
| `latest-unstable.json` | Unstable | Uniquement les builds non signés (pre-release / dev) |

- Les builds **signés** pointent vers `latest-stable.json` — les utilisateurs stable ne voient que des mises à jour signées
- Les builds **non signés** pointent vers `latest-unstable.json` — les testeurs restent sur le canal dev
- L'endpoint est configuré dans `tauri.conf.json` et réécrit en CI selon la disponibilité des secrets de signature

---

## Récapitulatif des secrets

| Secret | macOS | Windows | Updater |
|--------|:-----:|:-------:|:-------:|
| `APPLE_CERTIFICATE` | x | | |
| `APPLE_CERTIFICATE_PASSWORD` | x | | |
| `APPLE_SIGNING_IDENTITY` | x | | |
| `APPLE_ID` | x | | |
| `APPLE_PASSWORD` | x | | |
| `APPLE_TEAM_ID` | x | | |
| `CERTUM_OTP_URI` | | x | |
| `CERTUM_USER_ID` | | x | |
| `CERTUM_PASSWORD` | | x | |
| `TAURI_SIGNING_PRIVATE_KEY` | x | x | x |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | x | x | x |

---

## Coûts annuels

| Service | Coût |
|---------|------|
| Apple Developer Program | 99$/an |
| Certum EV Code Signing Cloud | ~226 EUR/an |
| **Total** | **~325$/an** |
