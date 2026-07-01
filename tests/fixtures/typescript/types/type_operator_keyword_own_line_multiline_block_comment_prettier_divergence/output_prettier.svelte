<script lang="ts">
	// An own-line multiline block comment in a prefix type operator's keyword->operand
	// gap hangs the operand on its own line, the comment kept where the author wrote it.
	// tsv diverges from prettier, which pulls the comment up onto the operator line and
	// leaves the operand flush. A GLUED multiline block collapses inline instead — see
	// keyword_value_glued_multiline_block_comment. Covers keyof, typeof, readonly.
	type K = keyof /* line1
		line2 */
	B;

	type Q = typeof /* line1
		line2 */
	x;

	type R = readonly /* line1
		line2 */
	A[];
</script>
