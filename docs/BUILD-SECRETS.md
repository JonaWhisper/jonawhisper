# Build Secrets & Environment Variables

Toutes les variables sont gerees par la CI (GitHub Actions).
Les builds locaux (`build.sh`) n'ont besoin d'aucun secret — le Keychain macOS gere la signature automatiquement.

## Secrets GitHub Actions

### macOS — Signature & Notarisation

| Secret | Description | Comment l'obtenir |
|--------|-------------|-------------------|
| `APPLE_CERTIFICATE` | Developer ID (.p12) en base64 | Exporter depuis Keychain Access, puis `base64 -i cert.p12 \| pbcopy` |
| `APPLE_CERTIFICATE_PASSWORD` | Mot de passe du .p12 | Choisi lors de l'export |
| `APPLE_SIGNING_IDENTITY` | Ex: `Developer ID Application: Nom (TEAM_ID)` | `security find-identity -v -p codesigning` |
| `APPLE_ID` | Email du compte Apple Developer | developer.apple.com |
| `APPLE_PASSWORD` | App-specific password | appleid.apple.com > Mots de passe pour les apps |
| `APPLE_TEAM_ID` | Team ID (10 caracteres) | developer.apple.com > Membership |

**Prerequis :** Compte Apple Developer Program (99$/an) + certificat "Developer ID Application"

### Windows — Signature du code

#### Option A : Certificat PFX

| Secret | Description | Comment l'obtenir |
|--------|-------------|-------------------|
| `WINDOWS_CERTIFICATE` | Fichier .pfx en base64 | `[Convert]::ToBase64String([IO.File]::ReadAllBytes("cert.pfx")) \| Set-Clipboard` |
| `WINDOWS_CERTIFICATE_PASSWORD` | Mot de passe du .pfx | Choisi lors de l'export |

Et dans `tauri.conf.json` > `bundle.windows` :
- `certificateThumbprint` : `Get-ChildItem Cert:\CurrentUser\My \| Format-Table Thumbprint, Subject`

#### Option B : Cloud HSM (recommande)

Pour Certum HSM ou SSL.com eSigner, utiliser `signCommand` dans `tauri.conf.json` :
```json
{ "bundle": { "windows": { "signCommand": "votre-commande %1" } } }
```
Les credentials du HSM sont stockes en secrets GitHub selon le fournisseur.

**Prerequis :** Certificat EV Code Signing (~226-500$/an)

### Auto-Updater — Signature des mises a jour

| Secret | Description | Comment l'obtenir |
|--------|-------------|-------------------|
| `TAURI_SIGNING_PRIVATE_KEY` | Contenu de `~/.tauri/jona-whisper.key` | `npx tauri signer generate -w ~/.tauri/jona-whisper.key` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Mot de passe de la cle | Choisi a la generation (vide si aucun) |

La cle publique est embarquee dans `tauri.conf.json` > `plugins.updater.pubkey`.

**IMPORTANT :** Ne jamais perdre la cle privee — sans elle, impossible de publier des mises a jour pour les installations existantes.

---

## Recapitulatif

| Secret | macOS | Windows | Updater |
|--------|:-----:|:-------:|:-------:|
| `APPLE_CERTIFICATE` | x | | |
| `APPLE_CERTIFICATE_PASSWORD` | x | | |
| `APPLE_SIGNING_IDENTITY` | x | | |
| `APPLE_ID` | x | | |
| `APPLE_PASSWORD` | x | | |
| `APPLE_TEAM_ID` | x | | |
| `WINDOWS_CERTIFICATE` | | x | |
| `WINDOWS_CERTIFICATE_PASSWORD` | | x | |
| `TAURI_SIGNING_PRIVATE_KEY` | x | x | x |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | x | x | x |

---

## Couts annuels

| Service | Cout |
|---------|------|
| Apple Developer Program | 99$/an |
| Certificat EV Windows (Certum) | ~226$/an |
| **Total** | **~325$/an** |
