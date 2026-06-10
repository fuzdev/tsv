<script>
	// Arrow in test position — needs parens (the arrow body would otherwise
	// absorb the ternary as `() => (1 ? x : y)`)
	const a=(()=>1)?x:y;

	// Yield in test position — needs parens
	function* gen(){
		const b=(yield 1)?x:y;
	}

	// Short control — a non-arrow/yield test needs no parens
	const c=cond?x:y;
</script>
