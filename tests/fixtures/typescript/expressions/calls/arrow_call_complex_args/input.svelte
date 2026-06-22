<script lang="ts">
	// Single object arg
	const a = arr.map((x) => fn({ a: x }));

	// Multiple complex args
	const b = arr.map((x) => fn({ a: x }, [x, x * 2], (y) => y));

	// Array arg only
	const c = arr.map((x) => fn([x, x * 2]));

	// Mixed simple and complex
	const d = arr.map((x) => fn(x, { a: x }, arr));
</script>
