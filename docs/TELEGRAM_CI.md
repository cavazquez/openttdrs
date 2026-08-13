# Notificaciones de commits por Telegram

El workflow [`telegram-commits.yml`](../.github/workflows/telegram-commits.yml)
se ejecuta en cada `push`. Recorre los commits recibidos en orden cronológico y
envía un mensaje por cada uno, con rama, SHA corto, asunto y enlace al commit.

No requiere modificar el código ni guardar credenciales en el repositorio. Para
activarlo en GitHub, crear estos *Actions secrets* del repositorio:

| Secret | Valor |
|---|---|
| `TELEGRAM_BOT_TOKEN` | Token del bot creado mediante [@BotFather](https://t.me/BotFather). |
| `TELEGRAM_CHAT_ID` | ID numérico del chat, grupo o canal destino. |

El bot debe poder escribir en ese destino; para un canal normalmente tiene que
ser administrador. Si falta alguno de los dos secretos, el workflow finaliza
correctamente sin enviar peticiones. Los valores nunca se imprimen en los logs.

Para obtener el `chat_id` de un chat privado, iniciar una conversación con el
bot y consultar `getUpdates` desde una sesión local segura. No subir el token a
un issue, commit, variable pública ni captura de pantalla.
