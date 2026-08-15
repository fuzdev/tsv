<script>
	// A run the author glued onto the header's line moves down to the body whole
	for (;;)
		/* b1 */ // b2
		fn();

	// A block on the header's line joins the run below when a later comment breaks it
	for (;;)
		/* b1 */
		// b2
		fn();

	// For-in - a line comment on the header's line takes its own line above the body
	for (const k in o)
		// b1
		fn();

	// For-of - a run glued onto the header's line moves down whole
	for (const k of o)
		/* b1 */ // b2
		fn();

	// For-of - a blank the author left between the run and the body survives
	for (const k of o)
		// b1

		fn();

	// Control - nothing forces a break, so a glued pair stays on the header's line
	for (const k of o) /* b1 */ /* b2 */ fn();
</script>
