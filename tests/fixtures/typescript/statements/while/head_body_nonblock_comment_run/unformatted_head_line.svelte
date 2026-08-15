<script>
	// A run the author glued onto the header's line moves down to the body whole
	while (a) /* b1 */ // b2
	fn();

	// A block on the header's line joins the run below when a later comment breaks it
	while (a) /* b1 */
	// b2
	fn();

	// A blank the author left between the run and the body survives
	while (a)
	// b1

	fn();

	// Do - a run glued onto the keyword's line moves down the same way
	do /* b1 */ // b2
	fn();
	while (a);

	// Do - a block on the keyword's line joins the run below
	do /* b1 */
	// b2
	fn();
	while (a);

	// Control - nothing forces a break, so a glued pair stays on the header's line
	while (a) /* b1 */ /* b2 */ fn();
</script>
