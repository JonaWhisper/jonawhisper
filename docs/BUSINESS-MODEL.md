# Business Model — JonaWhisper (Memli AI)

> Document de cadrage pour aligner le produit macOS avec la stratégie business.
> Dernière mise à jour : mars 2026

---

## 1. Vision produit

**JonaWhisper** est une app de dictée vocale macOS, locale et privée, propulsée par Whisper.
Le positionnement : **l'anti-SaaS de la dictation** — pas d'abonnement ruineux, pas de cloud, pas de compromis sur la vie privée.

**Builders :** Jonathan Philippe & Memli Sheremeti (indie devs).

---

## 2. Segments cibles

| Segment | Besoin principal | Sensibilité prix | Canal d'acquisition |
|---------|-----------------|-------------------|---------------------|
| **Développeurs** | Dicter du code/prompts dans Cursor, Claude, Terminal | Moyenne | Reddit, Twitter/X, Product Hunt |
| **Freelances / Indés** | Outil rapide, pas d'abo mensuel | Haute | Product Hunt, bouche-à-oreille |
| **Pros vie privée** (avocats, médecins, journalistes) | Zéro cloud, données sensibles | Basse | SEO, LinkedIn, partenariats |
| **Réfugiés Wispr Flow / SuperWhisper** | Déçus du prix ou de la vie privée | Haute | SEO comparatif, Reddit |

---

## 3. Proposition de valeur

```
┌─────────────────────────────────────────────────────┐
│  100% local   ×   Prix accessible   ×   Léger       │
└─────────────────────────────────────────────────────┘
```

| Axe | JonaWhisper | Wispr Flow | SuperWhisper |
|-----|-------------|------------|--------------|
| Audio dans le cloud | Non | Oui (+ screenshots) | Partiel ("AI mode") |
| Prix | €9/mois ou €79/an | $15/mois | $250 lifetime |
| Coût sur 3 ans | ~€237 (€79×3) | $540 | $250 |
| RAM | Léger | ~800 MB | Variable |
| Tier gratuit | Oui (illimité, modèles tiny/base) | Non | Limité |
| Langues | 90+ | ~30 | ~100 |
| Latence | 0.3s | Variable | Variable |

---

## 4. Modèle de revenus

### Tiers actuels

| Tier | Prix | Contenu |
|------|------|---------|
| **Free** | 0€ | Modèles Whisper tiny/base, raccourci global, 90+ langues, historique basique |
| **Pro** | €9/mois ou €79/an | Tous les modèles Whisper, 9 providers cloud (optionnel), VAD, ponctuation smart, cleanup LLM, historique illimité + recherche, support prioritaire |
| **Team** | €19/utilisateur/mois | Pro + vocabulaires partagés, console admin, facturation centralisée, onboarding custom |

### Métriques clés à suivre

- **Conversion Free → Pro** (cible : 5-8%)
- **Churn mensuel Pro** (cible : < 5%)
- **ARPU** (Average Revenue Per User)
- **LTV / CAC ratio** (cible : > 3)

---

## 5. Points de friction identifiés & ajustements produit

### 5.1 Le pricing est-il le bon ?

| Question | Analyse | Action suggérée |
|----------|---------|-----------------|
| €9/mois est compétitif vs Wispr ($15) mais reste un abo | Les users cibles détestent les abos (c'est notre propre argument marketing) | **Envisager un tier lifetime** (ex: €149 one-time) pour matcher le discours "anti-SaaS" |
| €79/an est attractif mais pas assez mis en avant | L'économie annuelle n'est pas claire visuellement | Afficher "€6.58/mois" sur le plan annuel |
| Le Free est généreux | Risque : users restent sur Free forever | Ajouter des limites douces (ex: historique 7 jours) pour pousser vers Pro |

### 5.2 Features manquantes pour monter en gamme

| Feature | Impact business | Effort | Priorité |
|---------|----------------|--------|----------|
| **Vocabulaire custom / termes techniques** | Fort — différenciateur pour devs et pros | Moyen | P1 |
| **Export / intégration** (Notion, Obsidian, clipboard formaté) | Fort — sticky pour freelances | Moyen | P1 |
| **Raccourcis par app** (profils contextuels) | Moyen — UX premium | Élevé | P2 |
| **Commandes vocales** ("nouveau paragraphe", "efface ça") | Fort — gap vs Wispr Flow | Élevé | P2 |
| **Mode dictée continue** (pas seulement push-to-talk) | Moyen — demandé par journalistes/rédacteurs | Moyen | P2 |
| **Widget menu bar** amélioré (stats, historique rapide) | Faible — nice-to-have | Faible | P3 |

### 5.3 Gaps dans le funnel

```
Découverte → Landing → Download → Install → First Use → Aha! → Pro → Rétention
    ▲            ▲         ▲          ▲          ▲        ▲       ▲
    │            │         │          │          │        │       │
  SEO/Reddit   CTA      .dmg ok?   Onboard?   Latence  Paywall  Churn?
```

| Étape | Friction | Fix produit |
|-------|----------|-------------|
| **Découverte** | Pas encore de SEO | Pages "JonaWhisper vs Wispr Flow", "best local dictation mac" |
| **Download** | .dmg = unsigned? Gatekeeper warning? | Signer l'app (Apple Developer ID) ou guider clairement |
| **First Use** | Le user ne sait pas quel modèle choisir | Auto-sélection du meilleur modèle selon le hardware |
| **Aha moment** | Dépend de la qualité de transcription | S'assurer que le modèle par défaut (base) est suffisant |
| **Conversion Pro** | Le Free est trop généreux | Time-limited trial des features Pro (7 jours) |
| **Rétention** | Pas de raison de revenir si "ça marche" | Historique, stats de productivité, streaks |

---

## 6. Stratégie de croissance

### Phase 1 — Traction (maintenant → 500 users)
- [ ] Product Hunt launch (angle : "lifetime alternative to Wispr Flow")
- [ ] Posts Reddit ciblés (r/macapps, r/productivity, r/speechrecognition)
- [ ] Thread Twitter/X viral : "Why I refuse to pay $15/month for dictation"
- [ ] SEO : pages comparatives optimisées

### Phase 2 — Monétisation (500 → 5 000 users)
- [ ] Introduire le tier **Lifetime** (€149) — l'arme fatale anti-SaaS
- [ ] Referral program (1 mois Pro gratuit par parrain)
- [ ] Partenariats créateurs de contenu tech (YouTube, newsletters)
- [ ] Témoignages / case studies (devs, avocats, médecins)

### Phase 3 — Scale (5 000+ users)
- [ ] Tier Team push vers startups et cabinets
- [ ] API / SDK pour intégrations tierces
- [ ] Marketplace de modèles / plugins communautaires
- [ ] Expansion : Windows, Linux (Tauri = cross-platform ready)

---

## 7. Risques & mitigations

| Risque | Probabilité | Impact | Mitigation |
|--------|-------------|--------|------------|
| Apple rend la dictée native aussi bonne | Moyenne | Fatal | Se différencier sur les features pro (vocabulaire, LLM cleanup, multi-modèle) |
| Wispr Flow baisse ses prix | Haute | Fort | Le local/privacy reste un moat inattaquable |
| Whisper est remplacé par un meilleur modèle | Moyenne | Moyen | Architecture modulaire — supporter plusieurs backends ML |
| Churn élevé sur l'abo mensuel | Haute | Fort | Pousser l'annuel et le lifetime |
| Gatekeeper / signature Apple bloque l'adoption | Moyenne | Fort | Investir dans l'Apple Developer Program ($99/an) |

---

## 8. Matrice Free vs Pro — feature par feature

> Chaque feature doit clairement appartenir à un tier. Si c'est flou pour nous, c'est flou pour l'utilisateur.

### 8.1 Features actuelles

| Feature | Free | Pro | Justification |
|---------|:----:|:---:|---------------|
| **Transcription locale (Whisper)** | tiny + base | tiny → large-v3 | Le Free doit être utilisable. Les gros modèles = upgrade naturel |
| **Raccourci global push-to-talk** | Oui | Oui | Core UX, ne pas gater |
| **90+ langues** | Oui | Oui | Argument marketing, ne pas limiter |
| **Détection auto de langue** | Oui | Oui | Coûte rien, améliore l'XP |
| **Historique de transcriptions** | 7 derniers jours | Illimité + recherche | Limite douce qui pousse vers Pro |
| **VAD (Voice Activity Detection)** | Non | Oui | Feature technique qui justifie le Pro |
| **Ponctuation intelligente** (47 langues) | Non | Oui | Différence de qualité immédiate et visible |
| **Cleanup LLM** (reformulation, correction) | Non | Oui | Feature "wow", justifie le prix |
| **Providers cloud** (9 options) | Non | Oui | Pour ceux qui veulent la meilleure qualité |
| **Support prioritaire** | Non | Oui | Classique, attendu |

### 8.2 Features à implémenter — TODO

Chaque feature est classée par tier cible et priorité d'implémentation.

#### P0 — Avant le launch (bloquant)

- [ ] **Signature Apple Developer ID** — *Infra*
  - Sans ça, Gatekeeper bloque l'install → abandon massif
  - Coût : $99/an Apple Developer Program
  - **Owner :** Memli
  - **Critère done :** .dmg s'installe sans warning Gatekeeper

- [ ] **Onboarding first-launch** — *Free*
  - Wizard 3 étapes : choisir raccourci → tester micro → première dictée
  - Auto-sélection du modèle selon hardware (M1/M2/Intel)
  - **Critère done :** un nouveau user fait sa première dictée en < 60 secondes

- [ ] **Paywall in-app clair** — *Free → Pro*
  - Quand un user Free touche une feature Pro → modal élégant avec upgrade
  - Pas de friction sur les features Free, juste un upsell naturel
  - **Critère done :** l'user comprend immédiatement ce qu'il gagne en passant Pro

#### P1 — Sprint post-launch (semaines 1-4)

- [ ] **Vocabulaire custom / termes techniques** — *Pro*
  - L'user ajoute des mots que Whisper transcrit mal (noms propres, jargon)
  - Stockage local dans un fichier JSON/SQLite
  - Impact : gros différenciateur pour devs et pros médicaux/juridiques
  - **Critère done :** l'user ajoute "JonaWhisper" et le modèle le transcrit correctement

- [ ] **Export clipboard formaté** — *Pro*
  - Copier avec formatage (Markdown, plain text, rich text)
  - Raccourci pour envoyer directement vers Notion / Obsidian / Notes
  - **Critère done :** l'user dicte → le texte arrive formaté dans Notion

- [ ] **Historique avec recherche full-text** — *Pro*
  - Recherche instantanée dans toutes les transcriptions passées
  - Filtres par date, langue, durée
  - **Critère done :** retrouver une transcription d'il y a 2 semaines en < 5 secondes

- [ ] **Trial Pro 7 jours** pour les nouveaux users — *Free → Pro*
  - À l'install, toutes les features Pro sont débloquées 7 jours
  - Après 7j → retour au Free avec modal "Vous avez utilisé X features Pro"
  - **Critère done :** conversion trial → paid > 10%

#### P2 — Mois 2-3

- [ ] **Commandes vocales** — *Pro*
  - "Nouveau paragraphe", "point", "efface la dernière phrase", "majuscule"
  - Parser local qui intercepte les commandes avant le paste
  - **Critère done :** les 10 commandes les plus courantes fonctionnent en FR et EN

- [ ] **Mode dictée continue** — *Pro*
  - Alternative au push-to-talk : le micro reste ouvert, VAD détecte les pauses
  - Pour les longues sessions (rédaction, compte-rendu)
  - **Critère done :** dicter 5 minutes sans toucher le clavier, avec des pauses naturelles

- [ ] **Raccourcis par app (profils contextuels)** — *Pro*
  - Quand Cursor est actif → modèle code-optimisé, pas de ponctuation auto
  - Quand Notes est actif → ponctuation + majuscules + cleanup
  - **Critère done :** le comportement change automatiquement selon l'app au premier plan

- [ ] **Stats de productivité** — *Free (basique) / Pro (complet)*
  - Free : "Vous avez dicté X mots aujourd'hui"
  - Pro : graphes, temps gagné vs frappe, streaks, export
  - But : rétention + argument "regardez combien vous gagnez"
  - **Critère done :** widget menu bar avec le compteur du jour

#### P3 — Nice-to-have (mois 4+)

- [ ] **Referral program** — intégré à l'app
  - "Invitez un ami → 1 mois Pro gratuit pour chacun"
  - Lien unique généré in-app
  - **Critère done :** flow complet invite → reward automatique

- [ ] **Widget menu bar avancé** — *Pro*
  - Mini historique, accès rapide aux 5 dernières transcriptions
  - Indicateur de modèle actif et langue détectée

- [ ] **Thèmes / personnalisation UI** — *Pro*
  - Dark/light/system + accent colors
  - Petit plaisir cosmétique qui fidélise

- [ ] **API locale / SDK** — *Team*
  - Pour les devs qui veulent intégrer JonaWhisper dans leur workflow
  - WebSocket local ou CLI

---

## 9. Décisions produit à prendre

> Ces questions doivent être tranchées pour aligner le développement.

1. **Lifetime pricing : oui ou non ?** Si oui, à quel prix ? (€99 / €149 / €199)
2. **Limiter le Free tier ?** (historique 7j, pas de VAD, modèle tiny uniquement)
3. **Priorité P1 : vocabulaire custom ou commandes vocales ?**
4. **Signing Apple :** investir maintenant dans le Developer ID ?
5. **Continuous dictation mode :** est-ce que ça cannibalise le push-to-talk ou ça le complète ?

---

*Ce document est vivant. Le mettre à jour après chaque décision produit majeure.*
