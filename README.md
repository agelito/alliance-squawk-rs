# Alliance Squawk

A discord bot which monitors alliance corporations and notifies a channel once a corporation
join or leave an alliance.

## Bot Permissions
This bot only requires the 'Send Message' permission. It does not respond to commands or read messages sent by users.

## Configuration

The bot is configured using environment variables. These are the available options:
| Variable          | Description                                                    | Required |
| ----------------- | -------------------------------------------------------------- | -------- |
| DISCORD_TOKEN     | The discord token.                                             | true     |
| NOTIFY_CHANNEL_ID | ID of the discord channel where notification should be posted. | true     |

The environment variables can be placed inside a `.env` file inside the application working directory.
