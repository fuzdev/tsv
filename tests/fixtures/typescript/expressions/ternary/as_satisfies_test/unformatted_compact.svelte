<script lang="ts">
// `as` in test position — prettier keeps clarity parens
const a=(x as T)?p:q;

// `satisfies` in test position — prettier keeps clarity parens
const b=(x satisfies T)?p:q;

// control — a plain test needs no parens
const c=cond?p:q;
</script>
