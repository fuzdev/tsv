<script lang="ts">
	// A `//` in both gaps of a retained `)!` shell: one shell, the leading comment on
	// the `(` line, the trailing one beside the operand, and an operand that breaks
	// inside it keeps its own continuation.
	x.m()
		.m()
		.c// c1
		(() => d) // t1
		!.f();
	// c2
	(a + // m
		b)! // t2
		.f();
</script>
