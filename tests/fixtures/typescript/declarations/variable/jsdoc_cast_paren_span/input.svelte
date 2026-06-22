<script>
	// JSDoc cast wrapping a call that expands inside the preserved parens
	const a = /** @type {A} */ (
		b.find((x) => x.type === 'cccc' && x.name === 'ddddddddddddddddddddddddddddddddddddd')
	);
	const e = a && typeof a.value === 'object';

	// Multiple consecutive multiline JSDoc casts
	const f = /** @type {A} */ (
		b.find(
			(x) => x.type === 'ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc'
		)
	);
	const h = 1;
</script>
