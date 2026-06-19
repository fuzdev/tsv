<script lang="ts">
// own-line block comment after the source arg (no options) — its own line
const a = import('./a'
/* c */);

// own-line line comment after the source arg (no options)
const b = import('./a'
// c
);

// own-line block comment after the options arg
const c = import('./a',x
/* c */);

// own-line line comment after the options arg
const d = import('./a',x
// c
);

// multiple own-line dangling comments after the options arg
const e = import('./a',x
/* c1 */
// c2
);
</script>
