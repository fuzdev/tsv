<style>
	/* between the `.` and a class name */
	./* c1 */cls {
		color: red;
	}

	/* between the `:` and a pseudo-class name */
	:/* c2 */hover {
		color: blue;
	}

	/* between the `::` and a pseudo-element name */
	::/* c3 */before {
		color: green;
	}

	/* between the two colons of a pseudo-element */
	:/* c4 */:before {
		color: teal;
	}

	/* the functional form: the name still ends at the `(` */
	:/* c5 */not(.a) {
		color: navy;
	}

	/* the case fold acts on the NAME, never on the comment's content */
	:/* C6 */hover {
		color: olive;
	}
</style>
