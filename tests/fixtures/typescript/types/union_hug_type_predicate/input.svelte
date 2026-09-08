<script lang="ts">
	// A brace-hugging union hugs the predicate's `is` — the object owns its
	// expansion, the void member trails the `}` — as it does at a return type.

	// Parameter predicate
	function single(a: unknown): a is {
		b: 1;
	} | null {}

	// Assertion predicate
	function withAsserts(a: unknown): asserts a is {
		b: 1;
	} | null {}

	// `this` predicate
	class Single {
		method(): this is {
			b: 1;
		} | null {}
	}

	// Arrow predicate
	const arrow = (
		a: unknown
	): a is {
		b: 1;
	} | null => true;

	// Null first, and a void mix
	function nullFirst(a: unknown): a is null | {
		b: 1;
	} {}
	function voidMix(a: unknown): a is {
		b: 1;
	} | null | void {}

	// A non-hugging union still breaks after `is`
	function nonHug(a: unknown): a is
		| {
				b: 1;
		  }
		| boolean {}
</script>
