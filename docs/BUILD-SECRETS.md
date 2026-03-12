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

### Windows — Signature via Certum Cloud HSM

Le certificat EV Code Signing est stocke dans le HSM cloud de Certum.
La signature se fait via SimplySign (emule une carte a puce) + `signtool.exe` sur le runner Windows.

#### Achat et activation

1. Acheter "EV Code Signing in the Cloud" sur [shop.certum.eu](https://shop.certum.eu/ev-code-signing-in-the-cloud.html) (~226 EUR/an)
2. Activer le certificat dans Mon compte > Data security products > Activate
3. Installer SimplySign Desktop sur une machine Windows
4. Se connecter et scanner le QR code (contient le seed TOTP pour l'automatisation CI)

#### Secrets GitHub

| Secret | Description | Comment l'obtenir |
|--------|-------------|-------------------|
| `CERTUM_OTP_URI` | URI otpauth extraite du QR code SimplySign | Scanner le QR code affiche lors de l'activation |
| `CERTUM_USER_ID` | Identifiant du compte Certum | Email ou ID numerique |
| `CERTUM_PASSWORD` | Mot de passe du compte Certum | Choisi a l'inscription |

#### Comment ca marche en CI

1. Le runner Windows installe SimplySign Desktop
2. Un script PowerShell genere le code TOTP a partir de `CERTUM_OTP_URI`
3. SimplySign s'active et emule une carte a puce
4. `signtool.exe` signe l'exe via la carte a puce virtuelle
5. Le timestamp server `http://timestamp.certum.pl` horodate la signature

#### Config Tauri (`tauri.conf.json`)

La config actuelle utilise `signtool.exe` qui detecte automatiquement le certificat
via SimplySign. Pas besoin de `signCommand` — Tauri appelle signtool nativement
quand `certificateThumbprint` est configure :

```json
{
  "bundle": {
    "windows": {
      "certificateThumbprint": "THUMBPRINT_DU_CERT_CERTUM",
      "digestAlgorithm": "sha256",
      "timestampUrl": "http://timestamp.certum.pl"
    }
  }
}
```

Le thumbprint est visible dans SimplySign Desktop ou via PowerShell :
`Get-ChildItem Cert:\CurrentUser\My | Format-Table Thumbprint, Subject`

#### References

- [Defguard: Tauri + Certum HSM](https://defguard.net/blog/windows-codesign-certum-hsm/)
- [Automatiser SimplySign en CI](https://www.devas.life/how-to-automate-signing-your-windows-app-with-certum/)
- [certum-container (Linux CI)](https://github.com/hpvb/certum-container)

### Auto-Updater — Signature des mises a jour

| Secret | Description | Comment l'obtenir |
|--------|-------------|-------------------|
| `TAURI_SIGNING_PRIVATE_KEY` | Contenu de `~/.tauri/jona-whisper.key` | `npx tauri signer generate -w ~/.tauri/jona-whisper.key` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Mot de passe de la cle (vide actuellement) | Choisi a la generation |

La cle publique est embarquee dans `tauri.conf.json` > `plugins.updater.pubkey`.

**IMPORTANT :** Ne jamais perdre la cle privee — sans elle, impossible de publier des mises a jour pour les installations existantes. Backup : `~/.tauri/jona-whisper.key`

---

## Recapitulatif des secrets

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

## Couts annuels

| Service | Cout |
|---------|------|
| Apple Developer Program | 99$/an |
| Certum EV Code Signing Cloud | ~226 EUR/an |
| **Total** | **~325$/an** |
