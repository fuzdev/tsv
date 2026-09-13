<script lang="ts">
	// a pair the printer RETAINS keeps the frozen operand inside it: the `as` operand→keyword
	// gap is ASI-sensitive, so a run that ends a line holds the pair open
	const a = // prettier-ignore
	{x:   1} as T;

	// `satisfies` answers the same way
	const b = // prettier-ignore
	{x:   2} satisfies T;

	// a non-null operand that needs precedence parens holds them for its own reason
	const c = // prettier-ignore
	(d  ??  e)!;

	// a chain base whose pair is REQUIRED — a sealed optional chain — owns its leading gap,
	// so the directive keeps the line the author gave it INSIDE the pair the chain prints
	// anyway, at a member lookup, a computed lookup, a call and a `!`
	const ff =
		(// prettier-ignore
		gg   ?.hh).ii;

	const jj =
		(// prettier-ignore
		kk   ?.ll)[0];

	const mm =
		(// prettier-ignore
		nn   ?.oo)();

	const pp =
		(// prettier-ignore
		qq   ?.rr)!.ss;

	// an IIFE base owns the same gap, and there prettier keeps the author's line too — the
	// one host in this fixture the two tools converge on
	(
		// prettier-ignore
		function   () {}
	)().tt;
</script>
