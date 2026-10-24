<script lang="ts">
	// a block glued to a grouping `(` stays glued to the value when the pair strips: nothing but
	// same-line blocks between them, or nothing at all
	async function fn() {
		await /* c1 */ (/* e1 */ a);
		await /* c2 */ (
		a);
		await /* c15 */ (a + // d15
			b);
	}
	new /* c4 */ (/* e4 */ A)();
	switch (x) {
		case /* c5 */ (/* e5 */ y):
			break;
	}
	function fn2() {
		return /* c6 */ (/* e6 */ a);
	}
	const v1 = /* c7 */ (/* e7 */ a);
	const v2 = /* c8 */ (
	a);
	fn(/* c9 */ (/* e9 */ a));
	const v3 = !(/* c10 */ (/* e10 */ a));
	const f1 = () => /* c11 */ (/* e11 */ a);
	const v4 = /* @__PURE__ */ (/* e12 */ fn());
	// a `//` after the pair's first token never parts the block from the value
	const v5 = /* c14 */ (a + // d14
	b);
	const f2 = /* c13 */ (
		// d13
		a
	) => a;
</script>
