<script lang="ts">
	// A comment on its own line before the `!` keeps its own line inside the
	// parens, below the operand, in authored order (a block following a `// c`
	// is always own-line — the `//` owns the rest of its line).
	const a = (x + y // c
	/* d */)!;

	// The parens are redundant here, and retained for the comment's sake.
	const b = (aaa
	// c
	)!;
</script>
