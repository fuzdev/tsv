<script>
	// In a module — always strict code — a leading `0` is a valid numeric literal only
	// before `.`, `e`, `n`, a radix prefix (`x`/`b`/`o`), or end, never before another
	// digit (the legacy-octal / non-octal-decimal forms strict code disallows). A `_`
	// separator after the leading `0` is a syntax error in every mode.
	const zero = 0;
	const frac = 0.5;
	const exp = 0e1;
	const big = 0n;
	const hex = 0x08;
	const bin = 0b0;
	const oct = 0o7;
	const after_dot = 0.08;
</script>
