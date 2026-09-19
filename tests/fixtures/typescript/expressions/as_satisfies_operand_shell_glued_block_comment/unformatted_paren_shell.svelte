<script lang="ts">
	// A single-line block comment glued to a cast's operand leads it inline, and the cast
	// takes no paren shell for it.
	const a = (/* c1 */ (x)) as A;

	// Same for `satisfies`.
	const b = (/* c2 */ (x)) satisfies B;

	// A chained cast.
	const c = (/* c3 */ (x)) as unknown as A;

	// Member, call and object operands.
	const d = (/* c4 */ (x.y)) as A;
	const e = (/* c5 */ (fn1())) as A;
	const f = (/* c6 */ ({ prop: 1 })) as const;

	// A block between the operand and the keyword rides along.
	const g = (/* c7 */ (x) /* c8 */) as A;

	// A bare expression statement.
	(/* c9 */ (x)) as A;

	// A postfix update's operand.
	(/* c10 */ (x))++;
	(/* c11 */ (x.y))--;

	// A restricted production's argument.
	function fn2() {
		return (/* c12 */ (x)) as A;
	}
	function fn3() {
		throw (/* c13 */ (x)) as A;
	}

	// An arrow body, a call argument, an array element and a template interpolation.
	const h = () => (/* c14 */ (x)) as A;
	fn4((/* c15 */ (x)) as A);
	const i = [(/* c16 */ (x)) as A];
	const j = `${(/* c17 */ (x)) as A}`;
</script>
