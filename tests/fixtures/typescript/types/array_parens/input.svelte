<script lang="ts">
	// Array types with parens needed
	type A = (A | B)[];
	type B = (A & B)[];
	type C = (() => T)[];
	type E = (typeof obj)[];

	// Array types - no parens needed
	type F = string[];
	type G = T[];
	type H = readonly T[];

	// Nested arrays
	type I = (A | B)[][];

	// keyof combinations
	type J = keyof (A | B);
	type K = keyof (A & B);

	// Conditional in array
	type L = (T extends U ? V : W)[];

	// Function types in array
	type M = (() => void)[];
	type N = ((x: T) => U)[];

	// Constructor types in array
	type O = (new () => T)[];
	type P = (abstract new () => T)[];
</script>
