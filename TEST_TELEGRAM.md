# 📱 Test des Boutons Telegram

Ce document explique comment tester les boutons interactifs (Start/Stop/Status) du bot Telegram.

## Prérequis

Assurez-vous que votre fichier `.env` contient les clés suivantes :
```bash
TELEGRAM_BOT_TOKEN=votre_token_ici
TELEGRAM_CHAT_ID=votre_chat_id_ici
```

## Lancer le Test

Pour tester l'intégration Telegram et les boutons, exécutez la commande suivante :

```bash
cargo run --features websocket -- test-telegram
```

## Ce qui va se passer

1. **Message de Test** : Le bot va envoyer un message texte simple "🔔 Test Notification".
2. **Clavier de Contrôle** : Le bot va envoyer un panneau de contrôle avec 3 boutons :
   - ▶️ Start
   - ⏹️ Stop
   - 📊 Status
3. **Mode Interactif** : Le programme va rester en écoute (ne se fermera pas tout de suite).

## Actions à Tester

Pendant que le programme tourne, cliquez sur les boutons dans votre application Telegram :

1. Cliquez sur **⏹️ Stop** :
   - Le bot doit répondre "🛑 Bot STOPPED".
   - Un message "🔴 Bot Stopped - Trading is paused" doit apparaître.

2. Cliquez sur **▶️ Start** :
   - Le bot doit répondre "✅ Bot STARTED".
   - Un message "🟢 Bot Started - Trading is now active" doit apparaître.

3. Cliquez sur **📊 Status** :
   - Le bot doit répondre avec l'état actuel (RUNNING ou STOPPED).

## Arrêter le Test

Une fois que vous avez vérifié que les boutons fonctionnent, vous pouvez arrêter le programme dans votre terminal avec `Ctrl+C`.

## En Cas de Problème

- Si vous ne recevez rien : Vérifiez votre `TELEGRAM_BOT_TOKEN` et `TELEGRAM_CHAT_ID`.
- Si les boutons ne répondent pas : Assurez-vous que le programme `cargo run` est toujours en cours d'exécution. Les boutons ne fonctionnent que si le bot est en ligne.
