# Workers [[workers:Component]]

- status: mvp
- type: background_workers

Background task workers.

## STT Worker [[stt_worker:Worker]]

- input: [[#File]]
- output: raw_transcript
- integration: [[#stt_api]]

Transcription of individual files. Returns text, timecodes, and diarization (speaker1..n).

## Merge Worker [[merge_worker:Worker]]

- input: [[#Task]]
- output: [[#Transcript]]
- integration: [[#llm_api]]
- triggers_after: all_files_transcribed

Merging the transcripts of all files into a single dialogue.

The LLM determines the logical order and merges into the format:
```
00:01:22 Alex: I disagree!
00:02:01 Sam: I don't mind either way!
```

## Summary Worker [[summary_worker:Worker]]

- input: [[#Transcript]]
- output: [[#Summary]]
- integration: [[#llm_api]]
- triggers_after: merge_complete

Generation of the automatic summary.

