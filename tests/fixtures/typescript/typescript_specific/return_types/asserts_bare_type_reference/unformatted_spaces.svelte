<script lang="ts">
	// `asserts` alone in return-type position is a plain type reference named
	// `asserts` — no parameter name or `this` follows, so it is not an
	// assertion predicate.
	function bare(x):   asserts   {}

	// `asserts` as the base of a regular type (array, generic, qualified).
	function array():   asserts[]   {}
	function generic():   asserts<T>   {}
	function qualified():   asserts.Foo   {}

	// Assertion predicates keep their `TSTypePredicate` shape (a parameter name
	// or `this` follows the `asserts` keyword).
	function predicate(x):   asserts   x   is   string   {}
	function thisPredicate(this: T):   asserts   this   is   T   {}

	// `asserts` as a plain type alias — the same reference outside return-type
	// position.
	type A   =   asserts;
</script>
