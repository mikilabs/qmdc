# S3 Storage [[s3:DataSource]]

- type: object_storage
- status: mvp

## Files Bucket [[s3_files:Bucket]]

- bucket_name: transcription-files
- lifecycle: 7_days
- encryption: AES-256

Storage of the source audio/video files.

After 7 days the files are deleted automatically (lifecycle policy).
Deleting the source moves the File to the `expired` status, but the transcript remains.

