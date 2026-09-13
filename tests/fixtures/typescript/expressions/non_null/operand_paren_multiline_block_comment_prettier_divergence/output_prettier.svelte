<script lang="ts">
	// A multiline block comment between a non-null operand and its `!` keeps the
	// grouping parens that hold it, and the shell expands, as a `//` makes it.
	let a = x /* m1
	m2 */!;

	// A single-line block ahead of it rides the same shell.
	let b = y /* c1 */ /* m3
	m4 */!;

	// A member access continues the chain past the `!`.
	const c =
		aaa /* m5
	m6 */!.bbb;

	// The operand is itself a call.
	const d =
		fn() /* m7
	m8 */!.ccc;

	// A bare optional chain, whose parens are otherwise redundant.
	const e =
		(p?.q /* m9
	m10 */)!;
	const f =
		(p?.() /* m11
	m12 */)!;

	// An operand that needs the parens anyway takes no second pair.
	const g = (h + i) /* m13
	m14 */!;

	// A single-line block comment stays inline without parens.
	let j = k /* c2 */!;
</script>
