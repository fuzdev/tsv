<script lang="ts">
	// Single-member union: comment between | and type
	let a1: /* c */ A;

	// Single-member intersection: comment between & and type
	let a2: /* c */ A;

	// Multiple comments between leading | and type
	let a3: /* c1 */ /* c2 */ A;

	// Parenthesized single-member union
	let a4: /* c */ A;

	// Multi-member union: comment before first member (let annotation)
	let a5: /* c */ A | B;

	// Multi-member intersection: comment before first member
	let a6: /* c */ A & B;

	// Multiple comments before first member
	let a7: /* c1 */ /* c2 */ A | B;

	// Multi-member with comments on multiple members
	let a8: /* c1 */ A | /* c2 */ B;

	// Const annotation
	const a9: /* c */ A | B = x;

	// Function parameter
	function fn1(a: /* c */ A | B): void {}

	// Arrow function parameter
	const fn2 = (a: /* c */ A | B) => {};

	// Type alias with leading |
	type B1 = /* c */ A;

	// Type alias with leading &
	type B2 = /* c */ A;

	// Function return type with leading |
	function fn3(): /* c */ A {}
</script>
