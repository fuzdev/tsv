<script lang="ts">
	// The `:` closes the ternary — the arrow after it is the alternate, not this paren's body
	const s1 = a ? (b, c) : (p) => 1;

	// The arrow sits past a second `?`, in the nested ternary's alternate
	const s2 = a ? b || (c ?? d) : e ? (p) => 1 : f;

	// The parenthesized consequent itself opens with `(`
	const s3 = z ? x || ((a, b) ?? c) : (p) => 1;

	// The `:` does annotate a signature here — plain return type
	const fn1 = (p): T => 1;

	// An annotated arrow as the ternary consequent
	const s4 = a ? (p): T => 1 : d;

	// Conditional type as the return type — its `?` and `:` belong to the type
	const fn2 = (p): A extends B ? C : D => e;
</script>
