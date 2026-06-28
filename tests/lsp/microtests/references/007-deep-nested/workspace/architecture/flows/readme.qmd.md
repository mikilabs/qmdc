# Flows [[flows:__Namespace]]

- description: End-to-end processes spanning multiple deliverables

## Task Processing [[task_processing:Flow]]

- status: mvp
- involves: [[#telegram_bot]], [[#backend]], [[#stt_worker]], [[#merge_worker]], [[#summary_worker]], [[#web_app]]

Processing a transcription task from upload to result.

### Sequence [[task_processing_steps:text]]

1. The user uploads files via [[#telegram_bot]]
2. [[#backend]] creates a Task and stores the files in S3
3. [[#stt_worker]] transcribes each file
4. [[#merge_worker]] merges the transcripts
5. [[#summary_worker]] generates the summary
6. The result is available in [[#web_app]]

## Payment Flow [[payment_flow:Flow]]

- status: mvp
- involves: [[#telegram_bot]], [[#backend]]

Payment via Telegram Stars.

### Sequence [[payment_steps:text]]

1. [[#telegram_bot]] detects the file durations
2. [[#backend]] calculates the price
3. [[#telegram_bot]] issues an invoice
4. The user pays
5. [[#backend]] starts processing

