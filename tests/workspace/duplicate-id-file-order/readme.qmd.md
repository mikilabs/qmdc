# Duplicate id file order [[dup_file_order: __Workspace]] Workspace

Every other file declares the same id. Which occurrence is reported as the duplicate, and which
one the query layer keeps, follows the order the files are read in: directory first, then
`readme.qmd.md`, then file name, each compared by UTF-8 bytes. The names differ only in case,
punctuation and directory, which is where locale collation and byte order disagree.
