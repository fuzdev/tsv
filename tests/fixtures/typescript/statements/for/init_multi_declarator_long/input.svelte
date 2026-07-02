<script>
	// Short multi-declarator for-init stays fully inline.
	for (let i = 0, j = 0; i < j; i++) {}

	// Multi-declarator for-init: declarators stay on one line when they fit within
	// print width at the broken-header indent (the `let …;` line is exactly 100).
	for (
		let aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = 0, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb = 0;
		cond;
		step
	) {}

	// One character longer (101): each declarator breaks onto its own line.
	for (
		let aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = 0,
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb = 0;
		cond;
		step
	) {}
</script>
