<script lang="ts">
	// Basic typed array destructuring
	const [a1]: [number] = arr;
	let [a2, b2]: [string, number] = arr;
	const [x, y, z]: [number, number, number] = arr;

	// With defaults
	const [a3 = 0]: [number?] = arr;
	let [a4 = 1, b4 = 2]: [number?, number?] = arr;

	// With rest
	const [a5, ...rest]: [number, ...number[]] = arr;

	// Nested tuple
	const [[a6]]: [[number]] = arr;

	// Mixed array/object in type
	const [x2]: [{a: number; b: string}] = arr;
</script>
