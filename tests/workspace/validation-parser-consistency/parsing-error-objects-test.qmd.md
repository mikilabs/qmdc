# Parsing Error Objects Test

Test to verify that `__ParsingError` objects do not participate in the `duplicate_id` check.

## Text Block

This is a text block with no fields and no `[[id]]`.

### Invalid Heading [[invalid_field:text]]

This heading with `[[id:type]]` inside a TextBlock must raise a `structured_in_textblock` error, but must not create an object that participates in `duplicate_id`.

## Another Text Block

Another text block.

### Another Invalid [[another_invalid]]

Another invalid heading inside a TextBlock.

Expected: `structured_in_textblock` errors, but NOT `duplicate_id` for `parsing_error_*` objects.
