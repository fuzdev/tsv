<script>
	// A lone block collapses onto the ; line
	if (a) /* c */ ;

	// A glued pair collapses the same way
	if (a) /* c1 */ /* c2 */ ;

	// A run whose later comment breaks takes the break with it
	if (a) /* c1 */
	// c2
	;

	// Two blocks the author gave separate lines keep them, and the ; joins the last
	if (a) /* c1 */
	/* c2 */
	;

	// A blank between two of them survives
	if (a) /* c1 */

	/* c2 */
	;

	// The else-to-empty-alternate gap answers identically
	if (a) fn();
	else /* c */ ;

	if (a) fn();
	else /* c1 */
	// c2
	;

	// while and for-of reach the same gap
	while (a) /* c */ ;

	for (const k of o) /* c1 */
	// c2
	;
</script>
