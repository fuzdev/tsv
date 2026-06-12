<script lang="ts">
	// Line comment trailing first type argument
	type T = Foo<A,// trailing
		B>;

	// Line comment trailing multiple type arguments
	type U = Bar<A,// trailing A
		B,// trailing B
		C>;

	// Trailing on last type argument
	type V = Baz<A,B// trailing
	>;
</script>
