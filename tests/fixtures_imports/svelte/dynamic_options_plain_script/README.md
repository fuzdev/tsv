A plain (non-TS) script: the canonical parser is vanilla acorn, which writes a dynamic import's second argument as `options`, not `arguments` — the record's `attributes` span it.
