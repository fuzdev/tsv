<style>
	/* a hex escape's terminator whitespace inside pseudo args is dropped before
	   the closing `)` (and before `,`), like other selector separators */
	:not(.\41 ) {
		color: red;
	}
	:is(.\41 ,.b) {
		color: blue;
	}
	/* pseudo-element args (a separate code path) drop the terminator before `)` too */
	::slotted(.\41 ) {
		color: teal;
	}
	/* an internal terminator that doubles as a descendant combinator is kept */
	:not(.\41 .b) {
		color: green;
	}
</style>
