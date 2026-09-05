<script>
	// A same-operator chain nested to the RIGHT is the same chain as the left-nested one, so it
	// prints identically: the array hugs its operator, and with an earlier operator to place the
	// chain takes a continuation indent. The `unformatted_paren_nested` variant writes every case
	// below with the redundant parens.
	const a = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
		cccccccccccccccccccccccccccccccccccccc ?? [1];

	// The same on an assignment expression, ending in an object
	a = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ||
		cccccccccccccccccccccccccccccccccccccc || { x: 1 };

	// Nested twice - every level of the same operator joins the one chain
	const b = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
		ccccccccccccccccccccccccc ??
		dddddddddddddddddddd ?? [1];

	// As an object property value and a class property initializer
	const obj = {
		prop: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
			cccccccccccccccccccccccccccccccccccccccc ?? [1]
	};
	class A {
		prop = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
			cccccccccccccccccccccccccccccccccccccc ?? [1];
	}

	// 100 chars - stays inline
	const d = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ?? ccccccccccccccccccccccccccccccccc ?? [1, 2];

	// 101 chars - the chain breaks
	const e = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
		cccccccccccccccccccccccccccccccccc ?? [1, 2];
</script>
