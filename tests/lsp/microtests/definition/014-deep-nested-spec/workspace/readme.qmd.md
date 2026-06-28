# Meeting Transcription Service [[transcription:__Workspace]]

- status: mvp
- version: 1.0
- languages: [ru, en]

A service for transcribing meetings over Telegram + Web.

A user sends meeting audio/video files to Telegram, pays a single invoice, and receives:
- a structured dialogue by speaker with timecodes
- an auto-summary
- a chat for working with the text
- version history
- export and public sharing

## Actors [[actors]]

### User [[user:Actor]]

- description: Owner of the transcripts
- auth: telegram

### Public Viewer [[viewer:Actor]]

- description: Access via a public link
- auth: none
- access: read-only
## Structure [[structure:text]]

- [[#deliverables]] — the system's deliverable components
- [[#storage]] — data sources
- [[#architecture]] — architecture, flows, domain

