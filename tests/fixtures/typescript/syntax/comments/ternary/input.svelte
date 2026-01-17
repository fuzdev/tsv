<script>
	const x = // after equals
		// before condition
		cond // inline
			? // before consequent
				a // inline consequent
			: // before alternate
				b; // inline alternate

	// Simple inline ternary with trailing
	const y = cond ? a : b; // trailing after ternary

	// Block comment variant
	const z = /* after equals */ /* before */ cond /* after cond */
		? /* before cons */ a
		: /* before alt */ b;
</script>
