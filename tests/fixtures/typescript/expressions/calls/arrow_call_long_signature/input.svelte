<script lang="ts">
	// Long signature with return type annotation
	const a = arr.map((x: A): B =>
		fn1({
			a: x,
			b: c
		})
	);

	// Shorter signature that fits
	const b = arr.map((x: T): U => fn({ a: x }));

	// Multiple params with types
	const c = arr.map((x: A, y: B): C => fn({ x, y }));

	// Very long type names: sig still hugs the open paren, body breaks after =>
	const d = arr.map((x: VeryLongTypeName): AnotherLongType =>
		fn2({
			a: x,
			b: c
		})
	);
</script>
