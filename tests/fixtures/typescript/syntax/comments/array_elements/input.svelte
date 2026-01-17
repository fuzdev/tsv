<script>
	// Simple trailing on first element
	const a = [A /* c1 */, B];

	// Comment after comma (leading on second element)
	const b = [A, /* c2 */ B];

	// Both patterns combined - the bug
	const c = [A /* c1 */, /* c2 */ B];

	// Leading on first element
	const d = [/* c1 */ A, B];

	// Multiple trailing
	const e = [A /* c1 */ /* c2 */, B];

	// Line comment after element
	const f = [
		A, // comment
		B,
	];

	// Line comment before element
	const g = [
		A,
		// comment
		B,
	];

	// Block and line mixed
	const h = [
		A /* block */,
		B, // line
		C,
	];

	// Line comment then block comment
	const i = [
		/* block */ A,
		B, // line
		C,
	];

	// Elision with comments
	const j = [A /* before elision */, , B];

	// Comment before closing bracket
	const k = [A, B /* before close */];

	// Empty array with comment
	const l = [
		/* inside empty */
	];

	// Spread with comments
	const m = [A, /* before spread */ ...B];

	// Comments on both sides of elision
	const n = [, /* before */ /* after */ B];
</script>
