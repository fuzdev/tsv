<script lang="ts">
// Single-member union collapses to its member, dropping the leading pipe
type A = | B;

// Single-member union wrapping a nested union collapses fully (no parens)
type C = | (D | E);

// Single-member union wrapping a function type drops the parens
type Fn = | (() => void);

// Single-member union wrapping an intersection drops the parens
type Inter = | (G & H);

// Single-member intersection collapses too
type M = & (N & O);
</script>
