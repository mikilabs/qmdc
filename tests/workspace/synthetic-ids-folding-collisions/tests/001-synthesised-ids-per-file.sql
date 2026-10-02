-- Every synthesised id is unique, and which file got which id is fixed by path byte order.
SELECT __id, __file FROM objects
WHERE __kind IN ('__Document', '__TextBlock') OR __id = 'doc_notes'
ORDER BY __id
