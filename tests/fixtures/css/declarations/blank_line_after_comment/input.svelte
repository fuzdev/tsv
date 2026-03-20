<style>
	/* Blank line after comment before nested rule preserved */
	span {
		/* comment */

		&::after {
			content: ' ';
		}
	}

	/* Blank line after comment before declaration preserved */
	div {
		/* comment */

		color: red;
	}

	/* No blank line after comment - stays compact */
	p {
		/* comment */
		color: red;
	}

	/* Blank line after comment between declarations preserved */
	a {
		color: red;
		/* comment */

		margin: 0;
	}
</style>
