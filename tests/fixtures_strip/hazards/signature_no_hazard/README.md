# signature_no_hazard

A blanked bodiless signature writes no `;` where nothing before it is left open: after a declaration ending in `}`, which no next line continues, and before `++` / `--`, ahead of which ASI ends the line anyway. The `;` of `overload_before_paren` goes in only where the program would otherwise change. An overload not followed by its implementation is the checker error TS2391, so the shape parses.

## Divergence

Node writes a `;` after the function declaration. The same program.
