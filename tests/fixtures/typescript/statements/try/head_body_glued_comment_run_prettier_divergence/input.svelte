<script>
	// Try - a glued pair between the keyword and the body
	try
	/* b1 */ /* b2 */
	{
	} catch (e) {}

	// Catch with param - a glued pair between ) and the body
	try {
	} catch (e)
	/* b1 */ /* b2 */
	{}

	// Bare catch - a glued pair between the keyword and the body
	try {
	} catch
	/* b1 */ /* b2 */
	{}

	// Finally - a glued pair between the keyword and the body
	try {
	} finally
	/* b1 */ /* b2 */
	{
	}

	// Try - a run ending in a line comment stays glued and still breaks
	try
	/* b1 */ /* b2 */ // b3
	{
	} catch (e) {}

	// Try - control, comments the author gave separate lines keep them
	try
	/* b1 */
	/* b2 */
	{
	} catch (e) {}
</script>
