<script lang="ts">
	// an own-line directive in an inter-operand gap freezes only the FOLLOWING operand
	fn(
		(a,
		// prettier-ignore
		b  (  1  ))
	);

	// the operands on either side of a frozen one still normalize
	fn(
		(a(0),
		// prettier-ignore
		b  (  1  ),
		c(2))
	);

	// an own-line block comment behaves identically — placement keys the freeze, not the spelling
	fn(
		(a,
		/* prettier-ignore */
		b  (  1  ))
	);

	// an assignment operand keeps its clarity parens around the frozen slice
	fn(
		(a,
		// prettier-ignore
		(b  =  1))
	);

	// a nested sequence operand prints its own grouping parens, which are not part of its
	// node span — the frozen slice re-synthesizes them or the operand loses its grouping
	fn(
		(a,
		// prettier-ignore
		(b,  c))
	);

	// a multi-line frozen operand keeps its verbatim layout; the `,` is parent-owned
	fn(
		(a,
		// prettier-ignore
		b(
			1,
				2
		),
		c)
	);
</script>
