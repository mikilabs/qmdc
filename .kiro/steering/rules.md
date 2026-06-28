---
inclusion: always
---
<!------------------------------------------------------------------------------------
   Add rules to this file or a short description and have Kiro refine them for you.
   
   Learn about inclusion modes: https://kiro.dev/docs/steering/#inclusion-modes
-------------------------------------------------------------------------------------> 

Use UV! 
Never use mutating git commands yourself! No git reset/checkout/stash/pop/commit/branch! Propose them to the user to make them by hand!

You can use info commands like git log/diff

Double- and triple-check multi-string commands, you are doing huge mistakes with quoting which hang zsh.

Prefer using qmdc mcp for information discovery over grepping/reading files 