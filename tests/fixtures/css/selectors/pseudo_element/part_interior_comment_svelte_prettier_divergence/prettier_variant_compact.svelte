<style>
	/* comment between two part identifiers */
	::part(name1/* c1 */name2) {
		color: red;
	}

	/* comment between the last two of three identifiers */
	::part(name1 name2/* c2 */name3) {
		color: blue;
	}

	/* two comments in one gap */
	::part(name1/* c3 */ /* c4 */name2) {
		color: green;
	}

	/* edge and interior comments together */
	::part(/* c5 */name1/* c6 */name2/* c7 */) {
		color: teal;
	}
</style>
