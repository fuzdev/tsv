<style>
	/* Test dimension edge cases: zero, negative, decimals, signs */
	div {
		/* Zero with and without units */
		margin: 0;
		padding: 0px;
		border-width: 0em;
		outline-width: 0%;
		/* Negative zero preserved like Prettier */
		right: -0px;
		left: -0rem;
		/* Positive zero preserved like Prettier */
		float: +0em;
		clear: +0%;
		/* Positive sign preserved like Prettier */
		z-index: +5;
		margin-left: +1.5px;
		max-width: +100em;
		min-height: +0.5rem;
		/* Negative values (lengths and percentages) */
		top: -10px;
		left: -2.5em;
		bottom: -1rem;
		margin-top: -100%;
		/* Decimal values */
		line-height: 1.5;
		font-size: 0.875rem;
		width: 2.5vw;
		/* Leading decimal point (prettier adds leading zero) */
		opacity: 0.5;
		padding-top: 0.25em;
		padding-left: 0.75rem;
		/* Trailing zeros */
		height: 1.5em;
		margin-bottom: 2px;
		/* Very small values */
		letter-spacing: 0.01em;
		border-width: 0.001px;
		/* Very large values */
		max-width: 9999px;
		min-height: 1000000px;
		/* Large percentages */
		transform: scale(1000%);
		/* Mixed in shorthand */
		padding: 0 10px 0.5em 1px;
		margin: 1px 0 2.5rem 0px;
	}
</style>
