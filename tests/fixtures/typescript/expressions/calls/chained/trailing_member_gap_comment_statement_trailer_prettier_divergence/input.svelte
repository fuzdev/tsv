<script lang="ts">
	// a same-line gap comment behind a statement trailer: the collapse would weld the pair
	fn() // c1
		.bar; // c2

	// the same rule in an initializer
	const a = fn() // c3
		.bar; // c4

	// and inside a call argument, the trailer past the closing paren
	foo(
		fn() // c5
			.bar
	); // c6

	// a block trailer: the collapse would reorder the pair, so the chain breaks here too
	fn() // c7
		.bar; /* c8 */

	// through an optional member and past a non-null on the call
	fn() // c9
		?.bar; // c10
	fn()! // c11
		.bar; // c12

	// past an array's `]` and an object's `}` — the collapse would weld there too
	const b = [
		fn() // c13
			.bar
	]; // c14
	const c = {
		k: fn() // c15
			.bar
	}; // c16

	// a block's `}` is read through as well. Here the trailer can never actually reach
	// the deferred comment's line (a `}` always takes one of its own), so this is the
	// rule firing conservatively — expansion that is unneeded, never wrong.
	function f() {
		fn() // c17
			.bar;
	} // c18

	// past a further token the closers read could not cross: a lookup's brackets, the
	// call the lookup is the callee of, then the trailer
	obj[
		fn() // c19
			.bar
	](); /* c20 */
	obj[
		fn() // c21
			?.bar
	](); // c22
	obj[
		fn()! // c23
			.bar
	](); /* c24 */
	const d =
		obj[
			fn() // c25
				.bar
		](); // c26

	// an arrow body, the trailer behind a lookup on the arrow — and one inside the
	// statement, ahead of a chain that continues past it
	(() =>
		fn() // c27
			.bar)[i]; // c28
	const e = (() =>
		fn() // c29
			.bar)[i]; /* c30 */
	(() =>
		fn() // c31
			.bar)[i] /* c32 */
		.baz()[i];

	// past an operator's right side and past a following sibling in a list
	fn() // c33
		.bar + a; /* c34 */
	foo(
		fn() // c35
			.bar,
		a
	); /* c36 */

	// the same two with a `//` trailer, which the collapse would push onto a line of its own
	fn() // c37
		.bar + a; // c38
	foo(
		fn() // c39
			.bar,
		a
	); // c40
</script>
