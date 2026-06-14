<style>
	/* pseudo-class and pseudo-element names are case-insensitive keywords, so
	   prettier lowercases them (the args are left alone — class names are
	   case-sensitive) */
	:HOVER {
		color: red;
	}
	::BEFORE {
		color: blue;
	}
	:NOT(.A) {
		color: green;
	}
	/* vendor-prefixed names lowercase too */
	::-WEBKIT-scrollbar {
		color: teal;
	}
	/* custom `:--name` pseudo-classes are preserved, not lowercased */
	:--Custom {
		color: orange;
	}
</style>
