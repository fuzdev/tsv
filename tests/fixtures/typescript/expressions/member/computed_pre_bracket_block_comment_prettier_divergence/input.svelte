<script lang="ts">
	// An own-line block comment between the object and the `[` of a computed access
	// keeps its own line, and the bracket follows on the next line.
	const a =
		arr
		/* c1 */
		[i];

	// A trailing block stays on the object's line; the own-line one keeps its line.
	const b =
		arr /* c2 */
		/* c3 */
		[i];

	// A run glued on one line stays before the `[` in both formatters.
	const c = arr /* c4 */ /* c5 */[i];

	// A comment glued after a multiline block also stays before the `[` in both.
	const d =
		arr /* m1
	m2 */ /* c6 */[i];

	// The optional form's `?.[` answers the same way.
	const e =
		arr
		/* c7 */
		?.[i];
</script>
