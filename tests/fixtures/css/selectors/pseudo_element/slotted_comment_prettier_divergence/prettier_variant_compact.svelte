<style>
	/* comment before the compound selector */
	::slotted(/* c1 */div) {
		color: red;
	}

	/* comment after the compound selector */
	::slotted(div/* c2 */) {
		color: blue;
	}
</style>
