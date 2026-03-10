# Auto-détection des providers cloud

## Concept

JonaWhisper peut détecter automatiquement des credentials stockées par d'autres outils AI installés sur la machine. L'utilisateur n'a rien à configurer — on scanne, on propose, il active.

Si une credential détectée utilise une API propriétaire (JetBrains AI, GitHub Copilot, etc.), on implémente le provider backend correspondant. La détection et le provider vont de pair.

### Principes

- **Consentement** : les providers détectés sont affichés mais désactivés par défaut. L'utilisateur choisit de les activer.
- **Lecture seule** : on ne modifie jamais les credentials d'une autre app.
- **Extensible** : chaque source de détection est un crate indépendant avec `inventory::submit!` (même pattern que les engines et providers).
- **API propriétaires bienvenues** : si une source détectée utilise une API non-standard, on crée le provider backend correspondant (`jona-provider-copilot`, `jona-provider-jetbrains`, etc.).
- **Même section UI** : les providers détectés apparaissent dans la même liste que les manuels, avec un badge "Auto".

### Architecture crate

Détecteur = trouve la credential. Provider = sait parler à l'API.

```
jona-types/src/provider.rs           ← DetectorRegistration + DetectedCredential
jona-detector-claude-code/            ← scanne keychain Claude Code → credential Anthropic
jona-detector-copilot/                ← scanne keychain gh/Copilot → credential Copilot
jona-detector-env/                    ← scanne variables d'environnement → multiples credentials
jona-provider-openai/                 ← backend OpenAI-compatible (existant)
jona-provider-anthropic/              ← backend Anthropic (existant)
jona-provider-copilot/                ← backend GitHub Copilot (token exchange + endpoint dédié)
jona-provider-jetbrains/              ← backend JetBrains AI (API propriétaire)
jona-provider/src/lib.rs              ← DetectorCatalog + ProviderCatalog
```

Ajouter une nouvelle source = créer un crate detector (+ un crate provider si API non-standard) + les ajouter dans `Cargo.toml`. Zéro modification ailleurs.

### Toggle enabled/disabled

Tous les providers (manuels ET détectés) ont un champ `enabled: bool`. Désactiver un provider = il reste visible mais n'est plus proposé dans les sélecteurs ASR/LLM.

---

## Recherche : sources de credentials détectables

### Méthodes de stockage identifiées

| Méthode | Description | Exploitable ? |
|---------|-------------|---------------|
| **Keychain direct** | Valeur en clair dans le trousseau macOS, protégée par ACL. Lecture avec prompt utilisateur. | ✅ Oui |
| **Electron Safe Storage** | Clé de chiffrement dans le Keychain + secrets chiffrés AES-256-GCM dans SQLite. | ❌ Non |
| **Fichiers config** | `.env`, `.yaml`, `.json` en clair sur le disque. Lisibles par tout processus. | ✅ Oui |
| **Variables d'environnement** | `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, etc. | ✅ Oui |

### Inventaire complet des outils AI

#### Keychain direct (exploitable)

| Outil | Type | Service Keychain | Account | Format valeur | API cible |
|-------|------|-----------------|---------|---------------|-----------|
| **Claude Code** | CLI (Node.js) | `Claude Code-credentials` | `<username macOS>` | JSON : `{"claudeAiOauth":{"accessToken":"sk-ant-oat01-..."}}` | Anthropic (OAuth, inclus dans l'abonnement Pro/Max/Team) |
| **IntelliJ AI Assistant** | IDE plugin | `IntelliJ Platform AI Assistant — Anthropic API Key` | `<NULL>` | Clé API brute `sk-ant-api03-...` | Anthropic (clé saisie manuellement par l'utilisateur) |
| **GitHub CLI** | CLI | `gh:github.com` | `<username>` | Token OAuth GitHub `gho_...` | GitHub API → potentiellement Copilot via token exchange |
| **Warp Terminal** | Terminal (Rust) | `dev.warp.Warp-Stable` | `User` | JSON (OAuth session Warp) | API Warp propriétaire (à investiguer) |
| **Gemini CLI** | CLI (Node.js) | Via `node-keytar` (nom exact non documenté) | Inconnu | OAuth Google | API Gemini (à investiguer) |

**Claude Code** :
- Token OAuth `sk-ant-oat01-*` lié à l'abonnement Claude. Appels inclus dans le plan.
- Expire ~8h, auto-refresh par Claude Code via `refreshToken`.
- Fallback : `~/.claude/.credentials.json` ou env var `CLAUDE_CODE_OAUTH_TOKEN`.
- Multi-profil : `Claude Code-credentials-{hash}` si `CLAUDE_CONFIG_DIR` est défini.

**IntelliJ** :
- Le service contient un em dash (`—`, U+2014), pas un tiret classique.
- Account = chaîne vide.
- Clé configurée manuellement par l'utilisateur dans l'IDE.
- JetBrains a aussi son propre service AI (AI Assistant) avec un token lié à la licence IDE. Ce token utilise une **API propriétaire JetBrains** — nécessiterait un `jona-provider-jetbrains` dédié. À investiguer : format du token, endpoints, capacités (LLM chat ? ASR ?).

**GitHub Copilot** :
- Le token `gh:github.com` peut être échangé contre un token Copilot via `api.github.com/copilot_internal/v2/token`.
- Le endpoint Copilot est `api.githubcopilot.com` — format OpenAI-compatible mais endpoint différent.
- Nécessite un `jona-provider-copilot` dédié : token exchange + requêtes vers l'endpoint Copilot.
- Prérequis : l'utilisateur doit avoir un abonnement GitHub Copilot actif.
- Projets open source de référence : `ericc-ch/copilot-api`, `hankchiutw/copilot-proxy`, LiteLLM `github_copilot/` provider.

#### Fichiers config (exploitable)

| Outil | Fichier(s) | Format | Clés détectables |
|-------|-----------|--------|-----------------|
| **Aider** | `~/.env`, `~/.aider.conf.yml`, `~/.aider/oauth-keys.env` | `.env` / YAML | `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GROQ_API_KEY`, etc. |
| **Continue.dev** | `~/.continue/config.yaml` ou `config.json` | YAML/JSON | Clés API multiples |
| **Mistral Vibe** | `~/.vibe/.env` | `.env` | `MISTRAL_API_KEY` |
| **Jan AI** | `~/jan/settings/@janhq/inference-openai-extension/` | JSON | Clés API par provider |
| **OpenRouter CLI** | `.env` (projet) | `.env` | `OPENROUTER_API_KEY` |
| **DeepSeek CLI** | `.env` | `.env` | `DEEPSEEK_API_KEY` |
| **Groq** | `~/.groq/local-settings.json` | JSON | Clé API |
| **AWS (Amazon Q)** | `~/.aws/sso/cache/*.json`, `~/.aws/credentials` | JSON/INI | Tokens SSO |

#### Variables d'environnement standard

| Variable | Provider |
|----------|----------|
| `OPENAI_API_KEY` | OpenAI |
| `ANTHROPIC_API_KEY` | Anthropic |
| `GROQ_API_KEY` | Groq |
| `GEMINI_API_KEY` | Gemini |
| `MISTRAL_API_KEY` | Mistral |
| `DEEPSEEK_API_KEY` | DeepSeek |
| `TOGETHER_API_KEY` | Together |
| `FIREWORKS_API_KEY` | Fireworks |
| `CEREBRAS_API_KEY` | Cerebras |
| `OPENROUTER_API_KEY` | OpenRouter |
| `CLAUDE_CODE_OAUTH_TOKEN` | Anthropic (OAuth) |

#### Electron Safe Storage (NON exploitable — chiffrement AES-256-GCM)

| Outil | Service Keychain |
|-------|-----------------|
| Cursor | `Cursor Safe Storage` |
| VS Code | `Code Safe Storage` |
| Windsurf | `Windsurf Safe Storage` |
| Claude Desktop | `Claude Safe Storage` |
| Cody (extension VS Code) | Via l'éditeur hôte |
| Figma | `Figma Safe Storage` |

#### Autres (non exploitable actuellement)

| Outil | Raison | Piste future ? |
|-------|--------|----------------|
| Warp | Token OAuth de session, pas une clé API provider directe | Oui — investiguer si les clés BYOK (OpenAI/Anthropic configurées dans Warp) sont dans le keychain |
| Raycast | App native Swift, stockage chiffré interne | Non |
| LM Studio | Local uniquement, pas de clés cloud | Non |
| Ollama | Local uniquement | Non |
| Tabnine | Via VS Code SecretStorage | Non |
| Perplexity | Abonnement, pas de clé API locale | Non |
| Pieces | Flutter `flutter_secure_storage`, pas documenté | Non |
| ChatGPT Desktop | SQLite parfois non chiffré, mais comportement instable | Peut-être — à surveiller |

---

## Priorisation pour l'implémentation

### Phase 1 — Keychain direct + infrastructure

**Infrastructure** (prérequis) :
- `DetectorRegistration` dans `jona-types` + `DetectorCatalog` dans `jona-provider`
- Champs `enabled` et `source` sur `Provider`
- UI : toggle enabled/disabled + badge "Auto"

**Détecteurs** :

| Crate | Source | Provider backend | Complexité |
|-------|--------|-----------------|------------|
| `jona-detector-claude-code` | Keychain `Claude Code-credentials` | `jona-provider-anthropic` (existant) | Faible |

### Phase 2 — Variables d'environnement

| Crate | Source | Provider backend |
|-------|--------|-----------------|
| `jona-detector-env` | `$OPENAI_API_KEY`, `$ANTHROPIC_API_KEY`, etc. | Providers existants |

### Phase 3 — APIs propriétaires

| Crate detector | Crate provider | API | Travail |
|---------------|---------------|-----|---------|
| `jona-detector-copilot` | `jona-provider-copilot` | GitHub Copilot (`api.githubcopilot.com`) | Token exchange + endpoint dédié. Réfs open source disponibles. |
| `jona-detector-jetbrains` | `jona-provider-jetbrains` | JetBrains AI Assistant | Investiguer l'API (endpoints, auth, capacités). |

### Phase 4 — Fichiers config

| Crate | Source |
|-------|--------|
| `jona-detector-dotenv` | `~/.env`, `~/.aider.conf.yml`, `~/.vibe/.env` |
| `jona-detector-continue` | `~/.continue/config.yaml` |

### Phase future — Investiguer

- Gemini CLI : trouver le service keytar exact dans le code source
- Warp : vérifier si les clés BYOK sont accessibles dans le keychain
- ChatGPT Desktop : surveiller la stabilisation du stockage

---

## Considérations techniques

### Prompt Keychain macOS

La lecture d'une entrée keychain d'une autre app déclenche un **prompt système** demandant à l'utilisateur d'autoriser l'accès. C'est le comportement attendu et souhaité (consentement explicite). L'utilisateur peut choisir "Autoriser" (une fois) ou "Toujours autoriser".

### Expiration des tokens OAuth

Les tokens OAuth (Claude Code) expirent (~8h). En cas de 401 :
- Log clair : "Token expiré — relancez Claude Code pour le rafraîchir"
- Le provider détecté reste visible mais inactif
- Re-détection au prochain démarrage

### Déduplication

Si l'utilisateur a déjà un provider manuel du même `ProviderKind`, le provider détecté est quand même affiché (c'est une source différente, un crate différent). L'utilisateur choisit lequel activer.

### Sécurité

- Les clés détectées ne sont **jamais** stockées dans les préférences JonaWhisper
- Elles restent en mémoire uniquement, re-détectées à chaque démarrage
- Les clés masquées (`••••xxxx`) sont affichées dans l'UI comme pour les providers manuels

### Tokens propriétaires

Pour les APIs propriétaires (Copilot, JetBrains), le crate provider implémente le protocole spécifique :
- **Copilot** : token exchange `gho_...` → token Copilot court-durée, puis appels OpenAI-format vers `api.githubcopilot.com`
- **JetBrains** : à investiguer — format du token, endpoints, scopes disponibles
