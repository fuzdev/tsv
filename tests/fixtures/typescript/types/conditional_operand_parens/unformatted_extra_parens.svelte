<script lang="ts">
// Check type keeps parens around function, constructor, and conditional types
type CkFn = ((() => void)) extends E ? X : Y;
type CkCtor = ((new () => T)) extends E ? X : Y;
type CkCond = ((A extends B ? C : D)) extends E ? X : Y;

// Check type needs no parens for union/intersection/keyof
type CkUnion = (A | B) extends E ? X : Y;
type CkKeyof = (keyof T) extends E ? X : Y;

// Extends type keeps parens around a nested conditional
type ExtCond = A extends ((B extends C ? D : E)) ? X : Y;

// Extends type needs no parens for a function type
type ExtFn = A extends (() => void) ? X : Y;
</script>
