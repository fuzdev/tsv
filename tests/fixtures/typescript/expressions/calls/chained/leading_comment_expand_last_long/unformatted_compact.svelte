<script>
// Short: inline JSDoc cast before expression arrow - fits on one line
a.b(x, /** @type {T} */ (y) => y);

// Block arrow: leading comment prevents expand-last, uses expand-all
a.b(x, /** @type {T} */ (y) => { z; });

// Object boundary 100: fits inline
a.b(x, /** @type {T} */ {aaaa: 1, bbbb: 2, cccc: 3, dddd: 4, eeee: 5, ffff: 6, gggg: 7, hh: 8});

// Object boundary 101: expands, leading comment prevents expand-last
a.b(x, /** @type {T} */ { aaaa: 1, bbbb: 2, cccc: 3, dddd: 4, eeee: 5, ffff: 6, gggg: 7, hh: 88 });
</script>
