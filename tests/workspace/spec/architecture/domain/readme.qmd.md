# Domain [[domain:__Namespace]]

- description: Conceptual model of the problem domain

## Task [[Task:Entity]]

- table: [[#tasks]]
- contains: [[#File]][]
- produces: [[#Transcript]], [[#Summary]]

A transcription task — 1 Task = 1 Telegram message with 1..N files.

### Statuses [[task_statuses]]

| status | description |
|--------|-------------|
| created | Task created |
| scanning_durations | Detecting durations |
| pricing | Calculating the price |
| invoice_sent | Invoice sent |
| waiting_for_payment | Awaiting payment |
| paid | Paid |
| processing_files | Processing files |
| merging | Merging transcripts |
| generating_summary | Generating summary |
| ready | Ready |
| expired | Source files deleted |
| error | Error |

## File [[File:Entity]]

- table: [[#files]]
- storage: [[#s3_files]]
- belongs_to: [[#Task]]

A single audio/video file.

### Statuses [[file_statuses]]

| status | description |
|--------|-------------|
| uploaded | Uploaded to S3 |
| duration_scanned | Duration detected |
| processing | Transcription in progress |
| transcribed | Transcribed |
| expired | Deleted from S3 |
| error | Processing error |

## Transcript [[Transcript:Entity]]

- table: [[#transcripts]]
- belongs_to: [[#Task]]
- has_many: [[#Version]][]

The final merged meeting text with timecodes and speakers.

### Features [[transcript_features:text]]

- Editing the text
- Renaming speakers
- Merging speakers
- Public access via a link

## Summary [[Summary:Entity]]

- belongs_to: [[#Task]]

An automatic summary. Generated once the transcript is ready. The user can regenerate it manually.

## Version [[Version:Entity]]

- table: [[#versions]]
- belongs_to: [[#Transcript]]

A transcript version. Linear history without branching. Autosave on every change. A rollback creates a new version.

