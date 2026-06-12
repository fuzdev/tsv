<script lang="ts">
	// Generic arrow functions with string literal constraints

	// Single string literal constraint
	const single = <T extends 'a'>(x: T) => x;

	// Union of string literals
	const union = <T extends 'a' | 'b' | 'c'>(x: T) => x;

	// Multiple type params with string constraints
	const multiple = <T extends 'a', U extends 'b'>(a: T, b: U) => [a, b];

	// Mixed with other types
	const mixed = <T extends 'a' | number>(x: T) => x;

	// With return type annotation
	const withReturn = <T extends 'a' | 'b'>(x: T): T => x;
</script>
