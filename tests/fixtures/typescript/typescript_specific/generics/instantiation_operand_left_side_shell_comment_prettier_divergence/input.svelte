<script lang="ts">
	// An own-line comment in the grouping parens of an instantiation's operand, or of that
	// operand's own leftmost node, leads the value: every value seam answers it as it answers the
	// same run without the type arguments.
	x =
		// c1
		a<T>;
	y =
		// c2
		a()<T>;
	const z =
		// c3
		x.a()[x]<T>;
	class K {
		p =
			// c4
			[]()<U>;
	}
	o = {
		k:
			// c5
			a.b<T>
	};
	w =
		/* c6 */
		a<T>;
	function f(
		b = // c7
			a<T>
	) {}
	async function g() {
		// `await` takes the run into its keyword gap, ahead of any pair the operand prints
		await // c8
			a<T>;
	}
	function h() {
		// a restricted production holds the run inside its hanging pair, for a block the author
		// broke after and a `//` alike
		return (
			/* c9 */
			a<T>
		);
	}
	function i() {
		return (
			// c10
			a<T>
		);
	}
	function* j() {
		yield (
			/* c11 */
			a<T>
		);
	}
	function k() {
		throw (
			/* c12 */
			a<T>
		);
	}
</script>
