# single_specifier_long_prettier_divergence

Prettier intentionally keeps single-specifier imports on one line even when they exceed printWidth (decision from 2017: "you don't get much more information when they are in two lines").

tsv: wraps at 101+ chars
Prettier: never wraps single-specifier imports

## Reason

tsv wraps consistently — if printWidth=100 is configured, lines should respect it. Multi-specifier imports already wrap at printWidth. The original Prettier decision received 18 thumbs-down vs 0 thumbs-up. Tooling (code review, terminals, side-by-side diffs) relies on line width limits.
