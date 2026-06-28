# Web App [[web_app:Component]]

- status: mvp
- type: spa
- framework: react
- auth: telegram_login
- depends_on: [[#backend]]

Web application for viewing, editing, and analyzing transcripts.

## Authorized Mode [[auth_mode:Mode]]

- actor: [[#user]]
- access: full

The full working interface for the owner of the transcripts.

### Tab: Transcript [[tab_transcript:Page]]

- features: view, edit, speakers, versions

View the dialogue with timecodes, rename and merge speakers, edit the text, version history with autosave.

### Tab: Summary [[tab_summary:Page]]

- features: view, regenerate

Auto-summary, choice of generation prompt, manual regeneration.

### Tab: Chat [[tab_chat:Page]]

- features: llm_dialog

A dialogue with the LLM about the transcript content. Requests: improve the summary, add details, explain a fragment.

## Public Mode [[public_mode:Mode]]

- actor: [[#viewer]]
- access: read-only
- requires: owner_permission

Access via a public link (if the owner allowed it):
- dialogue + summary + export only
- no login
- no editing

## Export [[export:Feature]]

- formats: [markdown, pdf, docx]
- available_in: [[#auth_mode]], [[#public_mode]]

