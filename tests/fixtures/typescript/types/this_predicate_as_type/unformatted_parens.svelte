<script lang="ts">
	// `this is T` is a valid type anywhere a type is expected — not only in return
	// position (unlike `x is T`, which the grammar restricts to return position).

	// Class setter parameter (the reported case)
	class C {
		set p(x: (this is string)) {}
	}

	// Interface setter
	interface I {
		set p(x: (this is string));
	}

	// Ordinary function parameter
	function f(x: (this is string)) {}

	// Variable type annotation
	let a: (this is number);

	// Type alias right-hand side
	type T = (this is string);

	// Nested as a type argument
	let b: Array<(this is boolean)>;
</script>
