<style>
	div {
		/* Comma-separated at exactly 100 - no wrapping */
		--list100: a0000000, b0000000, c0000000, d0000000, e0000000, f0000000, g0000000, h0000000, i000;

		/* Comma-separated at 101 - width-based wrapping */
		--list101: a0000000, b0000000, c0000000, d0000000, e0000000, f0000000, g0000000, h0000000, i0000;

		/* Space-separated with functions at exactly 100 - no wrapping */
		--shadow100: var(--a1) color-mix(in hsl, var(--b1) var(--b2), transparent), var(--c1) red000000;

		/* Space-separated with functions at 101 - width-based wrapping */
		--shadow101: var(--a1) color-mix(in hsl, var(--b1) var(--b2), transparent), var(--c1) red0000000;
	}
</style>
