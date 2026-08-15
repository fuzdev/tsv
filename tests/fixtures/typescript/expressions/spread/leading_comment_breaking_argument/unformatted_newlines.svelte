<script lang="ts">
	// A block run the author broke after, before a spread argument that breaks:
	// the run keeps the `...` line and the argument opens on the next.
	const a = [
		...
		/*#__PURE__*/
		fn1({
			b: 1,
			c: 2
		})
	];

	// The same rule in a call-argument position.
	fn2(
		...
		/*#__PURE__*/
		fn1({
			b: 1,
			c: 2
		})
	);

	// A glued run stays glued — the argument's break does not pull it apart.
	const d = [
		...
		/*#__PURE__*/ fn1({
			b: 1,
			c: 2
		})
	];

	// An argument that fits collapses the comment's soft separator to a space.
	const e = [.../* c */
	arr1];
</script>
