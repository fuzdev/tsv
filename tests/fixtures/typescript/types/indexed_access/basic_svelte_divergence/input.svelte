<script lang="ts">
	// Basic indexed access
	type A = T[K];
	type B = Obj['key'];

	// Nested indexed access
	type C = T[K][J];

	// With keyof
	type D = T[keyof T];

	// In return types
	const fn = <T, K extends keyof T>(obj: T, key: K): T[K] => obj[key];

	// In Promise
	const asyncFn = async <T, K extends keyof T>(obj: T, key: K): Promise<T[K]> => obj[key];

	// Array access
	type E = T[number];

	// Multiple index types
	type F = Obj['a' | 'b'];
	type G = T[K | J];

	// Conditional with indexed access
	type H = T[K] extends string ? T[K] : never;

	// With typeof
	type I = (typeof obj)['key'];
	type J = (typeof arr)[number];

	// Tuple indexed access
	type K = [string, number][0];
	type L = [string, number][1];

	// On union/intersection
	type M = (A | B)[K];
	type N = (A & B)[K];

	// In mapped type value
	type O<T> = { [P in keyof T]: T[P] };
</script>
