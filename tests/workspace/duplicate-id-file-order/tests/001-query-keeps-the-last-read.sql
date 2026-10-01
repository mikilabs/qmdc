-- The query layer keeps the occurrence read last, so this pins the read order: directory, then
-- readme.qmd.md, then file name, each by UTF-8 bytes. Which duplicate should win is not specified;
-- that every parser reads the files in one order is.
SELECT __file, json_extract(data, '$.status') AS status FROM objects WHERE __id = 'dup'
