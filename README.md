# Leaderboule

Classement et système d'Elo pour le club de pétanque de l'INSA de Lyon
Écrit en Rust avec Dioxus

## Comment exécuter

Les variables d'environnement suivantes doivent être accessibles

Soit présentes dans un `.env` soit setup depuis la CI/CD pipeline

```bash
# Pour la databse
DATABASE_URL="127.0.0.1:8000" # Développement uniquement
DATABASE_USER="root"
DATABASE_PASS="root"
# Pour l'intégration avec Helloasso
HELLOASSO_CLIENT_ID="..." 
HELLOASSO_CLIENT_SECRET="..."
# Seulement nécéssaire pour le déployement
SERVER_HOST="..."
SERVER_USER="..."
SSH_PRIVATE_KEY="..."
```


### Développement

- Installe le langage [Rust](https://rust-lang.org/tools/install/)
- Installe [SurrealDB](https://surrealdb.com/surrealdb/install)
- Installe [Dioxus](https://dioxuslabs.com/learn/0.7/getting_started/)

- Lance la databse :
```bash
./db_start.sh
```

- Lance l'application
```bash
dx serve
dx serve --addr 0.0.0.0 # Pour exposer sur le réseau local
dx serve --release # Pour maximum de perfs
```

La database sera stockée dans `./boules.db`

### Avec Docker

- Installe Docker et Docker Compose

```bash
docker compose up --build 
```

## Fonctionnalités
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

## Restant à faire
- [ ] Système de trophés
- [ ] Changer le logo
- [ ] CICD Pipeline QUI FONCTIONNE
- [ ] Working Elo system (implémenter le job d'Anaé)
- [ ] Documentation

Compter le nombre de lignes de code (j'aime bien faire ça)

`wc -l src/client/* src/api/* src/server/* assets/style/* src/main.rs src/init.sql`
