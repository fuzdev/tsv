<script lang="ts">
	// A `//` in both gaps of a retained `)!` shell: one shell, the leading comment on
	// the `(` line, the trailing one beside the operand, and an operand that breaks
	// inside it keeps its own continuation.
	( // c1
		x
			.m()
			.m()
			.c(() => d) // t1
	)!.f();
	( // c2
		a + // m
			b // t2
	)!.f();
</script>
