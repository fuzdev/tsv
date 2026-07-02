<style>
	/* comment before the direction */
	p:dir(/* c1 */ ltr) {
		color: red;
	}

	/* comment after the direction */
	p:dir(rtl /* c2 */) {
		color: blue;
	}

	/* comment before the language */
	span:lang(/* c3 */ en) {
		font-family: serif;
	}

	/* comment after the language subtag */
	span:lang(en-US /* c4 */) {
		font-family: sans-serif;
	}
</style>
