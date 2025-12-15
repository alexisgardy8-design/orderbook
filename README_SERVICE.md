# Installation du Service Systemd

Pour faire tourner le bot en permanence sur votre serveur, suivez ces étapes :

1. **Copier le fichier de service**
   Copiez le fichier `orderbook-bot.service` dans le répertoire systemd :
   ```bash
   sudo cp orderbook-bot.service /etc/systemd/system/
   ```

2. **Recharger systemd**
   Informez systemd du nouveau service :
   ```bash
   sudo systemctl daemon-reload
   ```

3. **Activer le service au démarrage**
   Pour que le bot démarre automatiquement au reboot :
   ```bash
   sudo systemctl enable orderbook-bot
   ```

4. **Démarrer le service**
   Lancez le bot maintenant :
   ```bash
   sudo systemctl start orderbook-bot
   ```

5. **Vérifier le statut**
   Vérifiez que tout fonctionne bien :
   ```bash
   sudo systemctl status orderbook-bot
   ```

6. **Voir les logs**
   Pour voir les logs en temps réel :
   ```bash
   journalctl -u orderbook-bot -f
   ```

## Alternative simple (Screen)

Si vous ne voulez pas utiliser systemd, vous pouvez utiliser `screen` :

1. Lancez une session screen :
   ```bash
   screen -S trading-bot
   ```

2. Lancez le bot :
   ```bash
   cargo run --release --features websocket trade
   ```

3. Détachez la session avec `Ctrl+A` puis `D`.
   Le bot continuera de tourner en arrière-plan.

4. Pour revenir à la session :
   ```bash
   screen -r trading-bot
   ```
