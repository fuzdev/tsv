<style>
	/* U+00A0 glued to a bare value stays inside it: one ident to css-syntax, a separator to parseCss */
	[ attr = value  ] {
		color: red;
	}

	/* ...and the case flag glued to the run stays glued */
	[attr=value i  ] {
		color: blue;
	}

	/* an ASCII space beside the run is a token separator and is kept, as one space */
	[attr=value    s ] {
		color: green;
	}

	/* after a quoted value the run is its own token; flush against the quote, before the flag */
	[attr = 'value'  ] {
		color: teal;
	}

	[attr='value'   i ] {
		color: gray;
	}

	/* behind the case flag, where the `]` is due */
	[attr='value'  s   ] {
		color: black;
	}
</style>
