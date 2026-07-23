<script lang="ts">
	// A redundant paren shell around a LATER union member holds a leading line comment
	// with a leading block (mixed) or a trailing block (trailing). The parens strip, so
	// the run renders between the `| ` members: block/line each on their own line before
	// the member's `| `, a trailing block stays inline after the member.
	type U1 = A | ((/* b */ // c
	B));

	type U2 = A | ((// c
	B /* t */));
</script>
