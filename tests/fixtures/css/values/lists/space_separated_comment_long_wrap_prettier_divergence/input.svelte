<style>
	div {
		/* mid comment, 100 columns with the `;`: fits */
		background: 1px /* c */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 2px;
		/* 101 columns: the `;` would be the 101st, so the last word wraps */
		background: 1px /* c */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			2px;
		/* the same boundary at a long property */
		prop000000000000000000000000000: 1px /* c */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 2px;
		prop000000000000000000000000000: 1px /* c */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			2px;
		/* a trailing comment is the fill item that wraps */
		background: 1px aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			/* c */;
	}
</style>
