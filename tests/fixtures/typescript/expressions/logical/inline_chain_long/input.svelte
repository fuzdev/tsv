<script>
	// A chain ending in a non-empty array hugs the array to its operator; with a same-precedence
	// left operand the earlier operators' continuation indents one level
	const a = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
		cccccccccccccccccccccccccccccccccccccc ??
		ddddddddddddd ?? [1];

	// The same on an assignment, ending in an object
	a = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ||
		cccccccccccccccccccccccccccccccccccccccc ||
		dddddddddddddddddd || { x: 1 };

	// And as an object property value and a class property initializer
	const obj = {
		prop: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
			cccccccccccccccccccccccccccccccccccccc ??
			ddddddddddd ?? [1]
	};
	class A {
		prop = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
			cccccccccccccccccccccccccccccccccccccc ??
			ddddddddddddd ?? [1];
	}

	// A two-operand chain has no continuation: the array hugs its operator and expands
	const c =
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ?? [
			1, 2, 3
		];

	// 100 chars - stays inline
	const d = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ?? ccccccccccccccccccccccccccccccccc ?? [1, 2];

	// 101 chars - the chain breaks
	const e = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
		cccccccccccccccccccccccccccccccccc ?? [1, 2];
</script>
