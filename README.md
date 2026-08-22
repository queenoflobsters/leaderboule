# Leaderboule

Je ferai le readme plus tard

### Quelles doivent être les variables d'environnement ?
```bash
# C'est un exemple
DATABASE_URL="127.0.0.1:8000" # pour le développement, sera overwrite par le docker-compose.yml
DATABASE_USER="root"
DATABASE_PASS="root"
HELLOASSO_CLIENT_ID="..."
HELLOASSO_CLIENT_SECRET="..."
```

### TODO
- [ ] Pretty login
- [ ] Achievements
- [ ] Changer le logo
- [ ] Page not found
- [ ] CICD Pipeline
- [ ] Working Elo system
- [ ] README
- [ ] Doc

### wc command
`wc -l src/client/* src/api/* src/server/* assets/style/* src/main.rs db_init.sql`
# surreal start --user root --pass root --bind 127.0.0.1:8000 rocksdb:boules.db
