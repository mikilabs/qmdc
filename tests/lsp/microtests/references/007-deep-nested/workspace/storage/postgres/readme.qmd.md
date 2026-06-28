# PostgreSQL [[postgres:DataSource]]

- type: relational_db
- status: mvp

## Users [[users:Table]]

- primary_key: id
- entity: [[#user]]

| column | type | description |
|--------|------|-------------|
| id | bigserial | PK |
| telegram_id | bigint | Telegram user ID |
| username | varchar | Telegram username |
| language | varchar(5) | Preferred language |
| free_minutes_used | integer | Used free minutes |
| created_at | timestamp | Registration time |

## Tasks [[tasks:Table]]

- primary_key: id
- foreign_keys: [[#users]]
- entity: [[#Task]]

| column | type | description |
|--------|------|-------------|
| id | uuid | PK |
| user_id | bigint | FK to users |
| status | varchar | Current status |
| total_duration | integer | Total seconds |
| price_stars | integer | Price in Stars |
| created_at | timestamp | Creation time |
| completed_at | timestamp | Completion time |

## Files [[files:Table]]

- primary_key: id
- foreign_keys: [[#tasks]]
- entity: [[#File]]

| column | type | description |
|--------|------|-------------|
| id | uuid | PK |
| task_id | uuid | FK to tasks |
| s3_key | varchar | S3 object key |
| duration | integer | Duration seconds |
| status | varchar | Processing status |

## Transcripts [[transcripts:Table]]

- primary_key: id
- foreign_keys: [[#tasks]]
- entity: [[#Transcript]]

| column | type | description |
|--------|------|-------------|
| id | uuid | PK |
| task_id | uuid | FK to tasks |
| content | text | Full transcript |
| speakers | jsonb | Speaker mapping |
| is_public | boolean | Public access |
| public_token | varchar | Public access token |

## Versions [[versions:Table]]

- primary_key: id
- foreign_keys: [[#transcripts]]
- entity: [[#Version]]

| column | type | description |
|--------|------|-------------|
| id | uuid | PK |
| transcript_id | uuid | FK to transcripts |
| content | text | Full content snapshot |
| created_at | timestamp | Version creation time |

