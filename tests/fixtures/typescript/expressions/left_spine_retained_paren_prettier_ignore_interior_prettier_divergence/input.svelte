<script lang="ts">
	// a pair the printer RETAINS keeps the frozen operand inside it: the `as` operand→keyword
	// gap is ASI-sensitive, so a run that ends a line holds the pair open
	const a = (
		// prettier-ignore
		{x:   1}
	) as T;

	// `satisfies` answers the same way
	const b = (
		// prettier-ignore
		{x:   2}
	) satisfies T;

	// a non-null operand that needs precedence parens holds them for its own reason
	const c = (
		// prettier-ignore
		d  ??  e
	)!;
</script>
