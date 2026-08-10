<script lang="ts">
	// A pair the author glued onto one line keeps that line in an end-of-body trailing run,
	// where a pair given two lines keeps two (body_trailing_run).

	// Class body.
	class A {
		a = 1;
		/* c1 */ /* c2 */
	}

	// Function body.
	function fn() {
		const a = 1;
		/* c1 */ /* c2 */
	}

	// Interface body.
	interface B {
		a: string;
		/* c1 */ /* c2 */
	}

	// Enum body.
	enum C {
		A
		/* c1 */ /* c2 */
	}

	// Namespace body.
	namespace D {
		const a = 1;
		/* c1 */ /* c2 */
	}

	// Type literal.
	type E = {
		a: string;
		/* c1 */ /* c2 */
	};

	// Object literal.
	const obj = {
		a: 1
		/* c1 */ /* c2 */
	};

	// Array literal.
	const arr = [
		a
		/* c1 */ /* c2 */
	];

	// Parameter list.
	function fn2(
		a
		/* c1 */ /* c2 */
	) {}

	// Type-parameter declaration.
	function fn3<
		T
		/* c1 */ /* c2 */
	>() {}

	// A blank line between the two still splits them (control).
	class F {
		a = 1;
		/* c1 */

		/* c2 */
	}
</script>
