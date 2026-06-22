<script lang="ts">
	// Object destructuring with named type
	export const fn1 = ({ a, b }: A): void => {};

	// Object destructuring with inline type
	const fn2 = ({ x }: { x: number }) => x;

	// Function declaration with destructuring
	function fn3({ y }: { y: string }): string {
		return y;
	}

	// Array destructuring with type
	const fn4 = ([a, b]: [number, string]) => a;

	// Nested destructuring
	const fn5 = ({ a: { b } }: { a: { b: number } }) => b;

	// With default value
	const fn6 = ({ a = 1 }: { a?: number }) => a;

	// Destructuring + rest with types
	const fn7 = ({ a }: A, ...rest: B[]): void => {};

	// Array destructuring + rest with type
	const fn8 = ([first, ...rest]: number[]): number => first;
</script>
