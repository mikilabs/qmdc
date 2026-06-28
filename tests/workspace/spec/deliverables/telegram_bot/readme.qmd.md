# Telegram Bot [[telegram_bot:Component]]

- status: mvp
- type: telegram_bot
- priority: high
- depends_on: [[#backend]], [[#s3_files]]

The Telegram bot is the main entry point for users.

## Welcome [[welcome:Command]]

- trigger: /start
- actor: [[#user]]

Shows:
- information about the service
- a link to support
- a language-switch button

## File Upload [[upload:Command]]

- trigger: media_message
- actor: [[#user]]
- creates: [[#Task]]
- storage: [[#s3_files]]

The user sends audio/video files in a single message.
The bot creates a Task and stores the files in S3.

## Payment [[payment:Command]]

- method: telegram_stars
- actor: [[#user]]
- free_minutes: 15

Issuing a Telegram Stars invoice.

### Pricing Formula [[pricing:text]]

```
Price = (duration × cost_STT) + (tokens × cost_LLM) + margin
```

## Statuses [[bot_statuses:Feature]]

- notifies: [[#user]]

The bot reports progress:
- invoice issued
- awaiting payment
- paid
- processing
- ready + link to the site

