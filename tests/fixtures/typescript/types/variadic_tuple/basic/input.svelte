<script lang="ts">
	// Basic spread in tuple
	type A = [...T];
	type B = [...string[]];

	// Multiple spreads
	type C = [...A, ...B];

	// Mixed with regular elements
	type D = [string, ...T, number];
	type E = [first: string, ...rest: T];

	// In function signatures
	const fn = <T extends unknown[]>(...args: [...T]): T => args as T;

	// With constraints
	type F<T extends unknown[]> = [...T, string];
</script>
