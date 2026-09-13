<script lang="ts">
	// A redundant paren shell around a LATER union member holds a leading run: a line
	// comment with a leading block (mixed) or a trailing block (trailing), or blocks
	// alone, one of them on its own line (blocks). The parens strip, so the run renders
	// between the `| ` members, keeping the author's own glue; a trailing block stays
	// inline after the member.
	type U1 = A | ((/* b */ // c
	B));

	type U2 = A | ((// c
	B /* t */));

	type U3 = A | ((/* b2 */
	/* c2 */
	B));
</script>
