<script lang="ts">
	// A `//` in the required pair's leading gap stays inside the parens and takes
	// the expanded shell — the comment on the `(` line, the operand indented, the
	// `)!` back out.
	const a = ( // c1
		x + y
	)!;

	// An own-line comment keeps its own line inside the shell.
	const b = (
		// c2
		p ? q : r
	)!;

	// The non-null-over-cast assignment target takes the same rendering.
	( // c3
		x as T
	)! = 1;
</script>
