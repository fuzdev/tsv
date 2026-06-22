<script lang="ts">
	// as expression with trailing multiline object
	fn({} as T, {
		prop1: 1,
		prop2: 2
	});

	// as expression with longer type name
	fn({} as Ttttttttttttttttttttttttttttt, {
		prop1: 1,
		prop2: 2
	});

	// satisfies expression with trailing multiline object
	fn({} satisfies T, {
		prop1: 1,
		prop2: 2
	});

	// angle-bracket type assertion with trailing multiline object
	fn(<T>{}, {
		prop1: 1,
		prop2: 2
	});

	// new expression with as expression first arg
	new Proxy({} as T, {
		get() {
			return x;
		}
	});

	// Multiple type assertions before trailing object
	fn({} as A, {} as B, {
		prop1: 1
	});

	// Array with type assertion
	fn([1, 2, 3] as const, {
		prop1: 1
	});

	// satisfies with array
	fn([1, 2] satisfies number[], {
		prop1: 1
	});

	// Non-null assertion wrapping object
	fn({}!, {
		prop1: 1
	});

	// Mixed: as + non-null
	fn({}! as T, {
		prop1: 1
	});

	// Mixed: non-null wrapping as
	fn(({} as T)!, {
		prop1: 1
	});

	// Deeply nested wrappers
	fn(({} as A as B)! as C, {
		prop1: 1
	});
</script>
