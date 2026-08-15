<script>
	// A run the author glued onto the header's line moves down to the body whole
	if (a)
		/* b1 */ // b2
		fn();

	// A block on the header's line joins the run below when a later comment breaks it
	if (a)
		/* b1 */
		// b2
		fn();

	// else-if takes the same path
	if (a) {
		fn();
	} else if (b)
		/* b1 */ // b2
		fn();

	// A blank the author left between the run and the body survives
	if (a)
		// b1

		fn();

	// A comment before else selects the other if printer, which asks the same gap the
	// same way - an own-line block keeps its line there too
	if (a)
		/* b1 */
		fn();
	// c1
	else {
		fn();
	}

	// Control - nothing forces a break, so a glued pair stays on the header's line
	if (a) /* b1 */ /* b2 */ fn();
</script>
