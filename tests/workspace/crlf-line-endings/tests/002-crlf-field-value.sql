-- QMD-90 (GitHub #23): CRLF is read as LF, so a multi-line field value keeps no `\r`.
SELECT __id, json_extract(data, '$.overview') AS overview
FROM objects
WHERE __id = 'doc'
