<style>
	/* :is() with syntax error - preserves invalid selector */
	div:is(.a, ., .b) {
		color: red;
	}

	/* :where() with syntax error - preserves invalid selector */
	span:where(.class1, [, .class2) {
		color: blue;
	}

	/* :is() with pseudo-element - preserved (contextually invalid) */
	p:is(.class3, ::before, .class4) {
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
	section:where(.class5, .class6:unknown-pseudo, .class7) {
		border: 1px solid black;
	}
</style>
