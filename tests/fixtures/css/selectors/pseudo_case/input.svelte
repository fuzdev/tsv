<style>
	/* pseudo-class and pseudo-element names are case-insensitive keywords, so
	   prettier lowercases them (the args are left alone — class names are
	   case-sensitive) */
	:hover {
		color: red;
	}
	::before {
		color: blue;
	}
	:not(.A) {
		color: green;
	}
	/* vendor-prefixed names lowercase too */
	::-webkit-scrollbar {
		color: teal;
	}
	/* custom `:--name` pseudo-classes are preserved, not lowercased */
	:--Custom {
		color: orange;
	}
</style>
