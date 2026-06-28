# Integrations [[integrations:__Namespace]]

- description: External integrations

## STT API [[stt_api:Integration]]

- type: speech_to_text
- status: mvp
- used_by: [[#stt_worker]]

External transcription service. Returns text, timecodes, and diarization.

## LLM API [[llm_api:Integration]]

- type: language_model
- status: mvp
- used_by: [[#merge_worker]], [[#summary_worker]], [[#tab_chat]]

Language model for merging transcripts, generating summaries, and the chat.

## Telegram API [[telegram_api:Integration]]

- type: messaging
- status: mvp
- used_by: [[#telegram_bot]]

Telegram Bot API for interacting with users.

## AWS S3 [[aws_s3:Integration]]

- type: object_storage
- status: mvp
- used_by: [[#s3_files]]

Storage of the source files.

