<script lang="ts">
	// a comment written last inside a redundant pair down the right edge of an operand whose
	// required pair the printer emits stays inside that pair, after the operand, ahead of a
	// block and a comment written after the inner pair, each on a line of its own
	k1 = -(
		a + b // c1
		/* x1 */ // d1
	);
	k2 = !(
		a + b // c2
		/* x2 */ // d2
	);
	k3 = typeof (
		a + b // c3
		/* x3 */ // d3
	);
	k4 = void (
		a + (b + c) // c4
		/* x4 */ // d4
	);
	async function fn1() {
		k5 = await (a + b); // c5
		/* x5 */ // d5
	}
	function* fn2() {
		k6 = yield a + b; // c6
		/* x6 */ // d6
	}
	export = a + b; // c7
	/* x7 */ // d7

	// a binary chain's operand keeps the comment ahead of the operator's run, its pair required
	// or not
	k8 =
		(a + b) * // c8
		/* x8 */ // d8
		2;
	k9 =
		a + b && // c9
		/* x9 */ // d9
		q;

	// a spread's argument keeps the comment ahead of the run written after the inner pair
	k10 = [
		...(a + b) // c10
		/* x10 */ // d10
	];
	fn3(
		...(a + b) // c11
		/* x11 */ // d11
	);
	o1 = {
		...(a + b) // c12
		/* x12 */ // d12
	};

	// an own-line comment in the inner pair, a nested pair and a `//` alone after the pair take
	// the same order
	k13 = -(
		a + b
		// c13
		/* x13 */ // d13
	);
	k14 = -(
		a + (b + c)
		// c14
		// d14
	);
	k15 = [
		...(a + (b + c))
		// c15
		// d15
	];

	// a block alone after the pair stays after the comment, in the order written, at a binary
	// chain's operand and a spread's argument too
	k16 = -(
		a + b // c16
		/* x16 */
	);
	k19 =
		a + b && // c19
		/* x19 */
		q;
	k20 = [
		...(a + b) // c20
		/* x20 */
	];

	// a `//` alone after the pair already follows on its own line, and with nothing written
	// after the pair the comment trails the statement
	k17 = -(
		a + b // c17
		// d17
	);
	k18 = typeof (a + b); // c18
	k21 = a + b && q; // c21
	k22 = [
		...(a + b) // c22
	];
	function* fn4() {
		k23 = yield a + b; // c23
	}
	k24 = -(/* l24 */ a + b); // c24
</script>
