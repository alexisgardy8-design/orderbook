# 🔐 Configuration Coinbase Exchange WebSocket

## Comment obtenir vos credentials ?

1. **Connectez-vous** à https://www.coinbase.com/settings/api
2. **Créez une nouvelle API Key** avec les permissions :
   - ✅ **View** (lecture seule suffit pour le WebSocket)
3. **Notez les 3 valeurs** :
   - `API Key` → mettez dans `API_KEY`
   - `API Secret` → mettez dans `SECRET_KEY` (déjà en base64)
   - `Passphrase` → mettez dans `PASSPHRASE`

## Format du .env

```bash
API_KEY=abcd1234efgh5678ijkl9012mnop3456
SECRET_KEY=YmFzZTY0X2VuY29kZWRfc2VjcmV0X2tleV9oZXJl==
PASSPHRASE=your_passphrase_here
```

## ⚠️ Important

- Ne **jamais commit** le fichier `.env` dans git
- Le `.env` est déjà dans `.gitignore`
- Utilisez uniquement des permissions **View** (pas besoin de Trade/Transfer)

## Tester la connexion

```bash
# Sans credentials → ticker channel (public)
cargo run --release --features websocket live

# Avec credentials → level2 channel (orderbook complet)
# Ajoutez vos credentials dans .env puis :
cargo run --release --features websocket live
```

## 🎯 Avantages du canal level2

- **Orderbook complet** avec tous les niveaux de prix
- **Mises à jour incrémentales** (snapshot initial + l2update)
- **~100-1000 updates/seconde** au lieu de 10
- **Profondeur du marché** pour détecter les gros ordres
- **Détection d'arbitrage** plus précise avec liquidité réelle
