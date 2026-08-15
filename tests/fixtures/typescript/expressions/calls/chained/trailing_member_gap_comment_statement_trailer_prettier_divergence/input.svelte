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
</script>
