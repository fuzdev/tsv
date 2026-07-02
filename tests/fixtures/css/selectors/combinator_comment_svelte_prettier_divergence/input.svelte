<style>
	/* descendant combinator gap */
	div /* c1 */ p {
		color: red;
	}

	/* child combinator, comment after */
	a > /* c2 */ b {
		color: blue;
	}

	/* child combinator, comment before */
	i /* c3 */ > em {
		color: green;
	}

	/* next-sibling combinator */
	h2 + /* c4 */ p {
		color: teal;
	}

	/* subsequent-sibling combinator */
	h2 ~ /* c5 */ p {
		color: navy;
	}

	/* column combinator */
	col || /* c6 */ td {
		color: maroon;
	}

	/* comment on both sides of the symbol */
	a /* c7 */ > /* c8 */ b {
		color: olive;
	}

	/* compound-internal glued (stays a compound) */
	.a/* c9 */.b {
		color: orange;
	}

	/* glued compound, type + class (stays a compound) */
	div/* c11 */.b {
		color: pink;
	}

	/* glued compound, class + pseudo-class (stays a compound) */
	.a/* c12 */:hover {
		color: gray;
	}

	/* glued compound run across three members (each boundary stays glued) */
	.a/* c13 */.b/* c14 */#id {
		color: aqua;
	}

	/* relative-selector leading combinator in :has() */
	:has(> /* c10 */ img) {
		color: purple;
	}
</style>
