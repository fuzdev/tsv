<style>
	/* :is() with syntax error - invalid selector skipped */
	div:is(.a, ., .b) {
		color: red;
	}

	/* :where() with syntax error - invalid selector skipped */
	span:where(.class1, [, .class2) {
		color: blue;
	}

	/* :is() with pseudo-element - kept in AST (valid syntax) */
	p:is(.class3, ::before, .class4) {
		margin: 10px;
	}

	/* :where() with pseudo-elements only - all kept (valid syntax) */
	h1:where(::before, ::after) {
		padding: 5px;
	}

	/* :is() with mixed valid/invalid/pseudo-element - syntax errors skipped, rest kept */
	article:is(.a, ., ::marker, .b:hover, [attr, .c) {
		background: gray;
	}

	/* :where() with unknown pseudo-class - kept (syntactically valid) */
	section:where(.class5, .class6:unknown-pseudo, .class7) {
		border: 1px solid black;
	}

	/* :is() with An+B in an invalid context - `of` is valid only in :nth-*(), so the
	   whole arg is a contextually-invalid selector and the forgiving list is empty */
	ul:is(2n of) {
		list-style: none;
	}

	/* :where() with a contextually-invalid An+B among valid classes - only the An+B is
	   skipped; the valid siblings are kept (the text is preserved verbatim on format) */
	ol:where(.class8, 2n of, .class9) {
		list-style: decimal;
	}
	/* a dropped forgiving item is kept verbatim except for a whitespace COLLAPSE, which
	   must not reach inside an escape: `\<TAB>` is a valid escape whose escaped code
	   point IS that tab (css-syntax-3 §4.3.4/§4.3.7), so the class is named `b<TAB>c`.
	   Collapsing it to a space silently renames the class */
	:is(.a > . > .b\	c) {
		color: teal;
	}
	/* the item's OUTER trim is escape-aware too: `.b\ ` ends in an escape whose payload
	   is that space, and trimming it strands the backslash onto the `)` that closes the
	   pseudo — output tsv's own parser would then reject */
	.x:is(.a > . > .b\ ) {
		color: olive;
	}
</style>
