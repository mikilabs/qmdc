# Cross-cutting [[cross_cutting:__Namespace]]

- description: Cross-cutting concepts of the system

## Localization [[localization:Architecture]]

- status: mvp
- languages: [ru, en]

### Telegram Bot [[localization_bot:text]]

Auto-detect + manual switch via `/lang`. Stores the locale in the DB.

### Web App [[localization_web:text]]

RU/EN toggle. The language is saved in the user's profile.

### Summary [[localization_summary:text]]

Generated in the language chosen by the user.

## Authentication [[authentication:Architecture]]

- status: mvp
- method: telegram_login

### Telegram Bot [[auth_bot:text]]

The user is identified automatically by their Telegram ID.

### Web App [[auth_web:text]]

Telegram Login Widget. After authorization — a JWT session.

## Versioning [[versioning:Architecture]]

- status: mvp
- model: linear

Linear change history of a transcript. Autosave on every change. A rollback creates a new version.

## Analytics [[analytics:Architecture]]

- status: mvp

We log: uploads, payments, durations, status transitions, edits, exports, views.

