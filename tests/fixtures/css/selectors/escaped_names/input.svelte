<style>
	/* identity escapes keep the backslash in the AST name */
	#\? {
		color: red;
	}
	.\@ {
		color: blue;
	}
	a\+b {
		color: green;
	}
	/* hex escapes decode (with optional whitespace terminator) */
	#\3A b {
		color: orange;
	}
	/* mixed identity and hex in one name */
	.a\?\41 b {
		color: purple;
	}
</style>
