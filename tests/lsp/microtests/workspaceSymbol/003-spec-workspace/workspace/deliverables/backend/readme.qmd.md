# Backend [[backend:Component]]

- status: mvp
- type: api_server
- depends_on: [[#postgres]], [[#s3_files]]

REST API server for all clients.

## Tasks API [[tasks_api:Endpoint]]

- path: /api/tasks
- methods: [GET, POST]
- entity: [[#Task]]

CRUD operations on transcription tasks.

## Transcripts API [[transcripts_api:Endpoint]]

- path: /api/transcripts
- methods: [GET, PUT]
- entity: [[#Transcript]]

Retrieving and editing transcripts.

## Versions API [[versions_api:Endpoint]]

- path: /api/versions
- methods: [GET, POST]
- entity: [[#Version]]

Version history and rollback.

## Summary API [[summary_api:Endpoint]]

- path: /api/summary
- methods: [GET, POST]
- entity: [[#Summary]]

Retrieving and regenerating the summary.

## Auth [[auth_endpoint:Endpoint]]

- path: /api/auth
- method: telegram_login
- actor: [[#user]]

