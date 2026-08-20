<style>
	/* a named prefix, comment on either side of the separator */
	svg/* c1 */|rect {
		color: red;
	}

	svg|/* c2 */rect {
		color: blue;
	}

	/* the universal prefix */
	*/* c3 */|div {
		color: green;
	}

	*|/* c4 */div {
		color: teal;
	}

	/* the explicit no-namespace prefix, with a type selector and with the universal */
	|/* c5 */div {
		color: navy;
	}

	|/* c6 */* {
		color: olive;
	}

	/* a `|` inside the comment content is not the separator */
	svg/* | */|rect {
		color: purple;
	}
</style>
