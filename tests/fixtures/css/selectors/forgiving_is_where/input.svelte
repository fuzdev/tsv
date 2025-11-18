<style>
	/* :is() with syntax error - preserves invalid selector */
	div:is(.a, ., .b) {
		color: red;
	}

	/* :where() with syntax error - preserves invalid selector */
	span:where(.valid, [, .also-valid) {
		color: blue;
	}

	/* :is() with pseudo-element - preserved (contextually invalid) */
	p:is(.text, ::before, .content) {
		margin: 10px;
	}

	/* :where() with all invalid - preserves all */
	h1:where(::before, ::after) {
		padding: 5px;
	}

	/* :is() with mixed valid/invalid - preserves all */
	article:is(.a, ., ::marker, .b:hover, [attr, .c) {
		background: gray;
	}

	/* :where() with unknown pseudo-class - kept (syntactically valid) */
	section:where(.foo, .bar:unknown-pseudo, .baz) {
		border: 1px solid black;
	}
</style>
