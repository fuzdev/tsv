<style>
	/* a part name is an ident, so its escapes stay verbatim: an escaped space is inside the one
	   name `name1 name2`, not a separator between two names */
	::part(name1\ name2) {
		color: red;
	}

	/* a hex escape and its terminator whitespace are one code point, `A` */
	::part(\41 name3) {
		color: blue;
	}

	/* an escaped newline code point (`\a`) is content of the name, never a raw line break */
	::part(name4\a) {
		color: green;
	}
</style>
