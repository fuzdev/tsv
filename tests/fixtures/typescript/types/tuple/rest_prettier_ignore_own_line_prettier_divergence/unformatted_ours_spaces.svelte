<script lang="ts">
	// An own-line directive in a tuple rest's `...`→type gap freezes the element type.
	// The directive keeps the line the author gave it, so the type hangs below it.
	type  A = [
		...
					// prettier-ignore
					{b:   1}[]
	];

	// After another element, and ahead of one.
	type  C = [
		d ,
		...
				// prettier-ignore
				{e:   2}[] ,
		f
	];

	// An own-line block comment behaves identically — placement keys the freeze, not the
	// spelling.
	type  G = [
		...
				/* prettier-ignore */
				{h:   3}[]
	];

	// A multi-line frozen slice keeps its authored lines.
	type  I = [
		...
				// prettier-ignore
				{
				j:   4;
			}[]
	];

	// A composite operand declines the head freeze — the directive is adjacent to the union,
	// so the member rule claims it — and the head still keeps the directive's line.
	type  K = [
		...
				// prettier-ignore
				L | M
	];

	// A directive the author glued to the `...` is inert — the comment keeps the line it was
	// written on and the type normalizes.
	type  N = [
		... // prettier-ignore
		{ o: 6 }[]
	];
</script>
