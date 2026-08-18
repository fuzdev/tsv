<script lang="ts">
	// A comment in an IIFE callee's required parens stays inside them and takes the
	// expanded shell — the `//` on the `(` line, the function one indent in, the `)`
	// back out. The tagged-template tag position answers the same gap.
	(
		// c1
		() => {}
	)();
	const a = (
		// c2
		function () {}
	)();
	(
		// c3
		() => {}
	)`tpl`;

	// An own-line comment keeps its own line inside the shell; an optional call is
	// the same position.
	(
		// c4
		async () => {}
	)?.();

	// An inline block run leads the function flat inside the pair.
	(
		/* c5 */ () => {}
	)();

	// A callee whose required pair is not a function keeps the hoist — the run leads
	// the enclosing position, matching prettier.
	new // c6
	(function () {})();
	// c7
	(class A {})();

	// The pair owns its TRAILING gap too — a comment between the function and the `)`
	// stays inside the parens, where the author wrote it. A block run keeps the pair
	// flat; a `//` expands it, the closer dropping below the run.
	(
		() => {} /* c8 */
	)();
	(
		function () {} // c9
	)();
	(
		() => {} /* c10 */
	)().p;
	(
		() => {} /* c11 */
	)`tpl`;

	// With BOTH gaps commented the pair still takes ONE expanded shell — the leading run
	// above the function, the trailing run beside it, the `)` back out — which is
	// prettier's own shape here.
	(
		// c14
		() => {} /* c15 */
	)();
	(
		// c16
		async () => {} // c17
	)();

	// A comment written OUTSIDE the pair stays outside it, on the callee→`(` gap, and
	// so does one in a required pair that is not a function's.
	(
		() => {} /* c12 */
	)();
	(a ? b : c) /* c13 */();
</script>
