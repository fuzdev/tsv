<script lang="ts">
	// Basic - empty params
	const f1 = (): {a: number; b: number} => ({a: 1, b: 2});

	// With untyped param
	const f2 = (x): {a: number; b: number} => ({a: x, b: x});

	// With typed param
	const f3 = (x: number): {a: number; b: number} => ({a: x, b: x});

	// Destructured object param
	const f4 = ({x}): {a: number; b: number} => ({a: x, b: x});

	// Destructured array param
	const f5 = ([x]): {a: number; b: number} => ({a: x, b: x});

	// Generic arrow
	const f6 = <T extends object>(): {a: number; b: number} => ({a: 1, b: 2});

	// Nested object with semicolons
	const f7 = (): {nested: {a: number; b: number}} => ({
		nested: {a: 1, b: 2},
	});

	// Object inside generic type param
	const f8 = (): Promise<{a: number; b: number}> => Promise.resolve({a: 1, b: 2});

	// Index signature + property
	const f9 = (): {[k: string]: number; length: number} => ({length: 0});

	// As callback argument
	const arr: number[] = [];
	const mapped = arr.map((x): {a: number; b: number} => ({a: x, b: x}));

	// IIFE
	const result = ((): {a: number; b: number} => ({a: 1, b: 2}))();

	// Exported
	export const f10 = (): {a: number; b: number} => ({a: 1, b: 2});
</script>
