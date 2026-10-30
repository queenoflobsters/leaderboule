# Leaderboule

Classement et système d'Elo pour le club de pétanque de l'INSA de Lyon
Écrit en Rust avec Dioxus, PostgreSQL et SeaORM.

## Comment exécuter

Les variables d'environnement suivantes doivent être accessibles

Soit présentes dans un `.env` soit setup depuis la CI/CD pipeline

```bash
# Pour la base de données
DATABASE_URL="postgres://leaderboule:leaderboule@127.0.0.1:5432/leaderboule"
POSTGRES_USER="leaderboule"
POSTGRES_PASSWORD="leaderboule"
POSTGRES_DB="leaderboule"
# Pour l'intégration avec Helloasso
HELLOASSO_CLIENT_ID="..."
HELLOASSO_CLIENT_SECRET="..."
HELLOASSO_SLUG="petanqu-insa-club"
HELLOASSO_FORM_SLUG="adhesion-2025-2026-petanqu-insa-club"
# Emails séparés par des virgules, toujours administrateurs
ADMIN_EMAILS="admin1@example.com,admin2@example.com"
```

Le schéma est créé automatiquement au premier accès à la base
(voir `src/init.sql` et `src/server/db.rs`).

Les emails listés dans `ADMIN_EMAILS` sont forcés au rôle `admin` : ils ne
peuvent pas être rétrogradés depuis l'interface, même par un autre
administrateur. En développement, relancez le serveur après modification de
cette variable.

### Développement

- Installe le langage [Rust](https://rust-lang.org/tools/install/)
- Installe [Docker](https://docs.docker.com/engine/install/) ou Podman
- Installe [Dioxus](https://dioxuslabs.com/learn/0.7/getting_started/)

- Lance PostgreSQL :
```bash
docker compose up -d postgres
# ou, avec Podman
podman-compose up -d postgres
```

- Lance l'application
```bash
dx serve
dx serve --addr 0.0.0.0 # Pour exposer sur le réseau local
dx serve --release # Pour maximum de perfs
```

Les données sont conservées dans le volume Docker `postgres_data`.

### Avec Docker / Podman

```bash
docker compose up --build
# ou
podman-compose up --build --force-recreate
```

L'application est disponible sur `http://localhost:8080` et PostgreSQL sur
`localhost:5432`.

> `podman-compose` ne recrée pas un conteneur quand seule l'image est
> reconstruite : il compare la configuration du service, pas l'identifiant de
> l'image. Utilisez donc `--force-recreate` après une reconstruction, ou
> `podman-compose down` suivi de `podman-compose up --build`.

Les ports exposés sur la machine hôte sont configurables, ce qui est pratique
si `dx serve` ou une autre instance PostgreSQL tourne déjà :

```bash
APP_PORT=8081 POSTGRES_PORT=5433 podman-compose up --build
```

Il faut alors adapter `DATABASE_URL` pour `dx serve` :

```bash
DATABASE_URL="postgres://leaderboule:leaderboule@127.0.0.1:5433/leaderboule" dx serve
```

## Fonctionnalités
- [x] Documentation
- [x] Page de Classement
  - [x] Trier par différents critères
  - [x] Chercher un utilisateur
- [x] Enregistrer une partie
- [x] Historique des parties
- [x] Page de Compte
  - [x] Statistiques
  - [x] Déconnexion
  - [x] Changer le nom d'utilisateur
  - [x] Changer le mot de passe
- [x] Authentification
  - [x] Compte utilisateur
  - [x] Mot de passe (plutôt sécurisé)
  - [x] Sessions enregistrées sur le navigateur
  - [x] Intégration avec Helloasso
- [x] Système d'Elo custom pour la pétanque (voir [ELO_SYSTEM.md](ELO_SYSTEM.md))
  - [x] Prise en compte de l'écart au score et des parties terminées
  - [x] Gestion des équipes de 1 à 6 joueurs et de l'hétérogénéité
  - [x] Placement dynamique des nouveaux joueurs
- [x] Modération et administration
  - [x] Rôles `user` / `admin` (admins `.env` toujours admins)
  - [x] Recherche et inspection des comptes
  - [x] Édition de l'Elo et des compteurs avec validation
  - [x] Changement forcé / assainissement du nom d'utilisateur
  - [x] Réinitialisation du mot de passe d'un utilisateur
  - [x] Suspension, bannissement, promotion / rétrogradation
  - [x] Journal d'audit et historique d'activité
  - [x] Correction et suppression de parties avec recalcul des effets

## Restant à faire
- [ ] Système de trophés
- [ ] Changer le logo

Compter le nombre de lignes de code (j'aime bien faire ça)

`wc -l src/client/* src/api/* src/server/* src/server/entities/* assets/style/* src/main.rs src/init.sql`
