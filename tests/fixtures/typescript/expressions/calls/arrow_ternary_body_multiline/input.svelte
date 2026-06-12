<script>
	// Ternary with block arrow consequent
	const a = fn(() =>
		cond
			? ({x}) => {
					if (x) {
						add(x);
					} else {
						remove(x);
					}
				}
			: null,
	);

	// Ternary with block arrow alternate
	const b = fn(() =>
		cond
			? null
			: ({x}) => {
					if (x) {
						add(x);
					} else {
						remove(x);
					}
				},
	);

	// Simple - no block bodies, should stay flat
	const c = fn(() => (cond ? true_val : false_val));
</script>
