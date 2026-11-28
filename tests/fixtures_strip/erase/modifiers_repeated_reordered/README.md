# modifiers_repeated_reordered

A repeated or reordered modifier run blanks every TypeScript keyword in it, each occurrence of a repeat included; `static` stays wherever it stands. tsc's parser takes both forms (the order and repeat rules are its checker's), so the strip does too. The open field before a computed key takes the `;` on the first blanked keyword.
