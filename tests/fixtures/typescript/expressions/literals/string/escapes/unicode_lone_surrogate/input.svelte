<script lang="ts">
	// A lone surrogate is well-formed grammar in both escape forms — only the
	// well-formed-unicode proposals care. The 4-digit form already parses
	const lead4 = '\uD800';
	const trail4 = '\uDC00';

	// The `\u{}` form takes the same code points
	const lead = '\u{D800}';
	const trail = '\u{DC00}';
	const template = `\u{D800}`;

	// A well-formed PAIR is unaffected in EVERY spelling — the two halves
	// combine into one code point before either is judged lone, so the braced
	// and mixed forms decode like the literal and the 4-digit pair
	const pair4 = '\uD83D\uDE00';
	const pairBraced = '\u{D83D}\u{DE00}';
	const pairMixedLead = '\uD83D\u{DE00}';
	const pairMixedTrail = '\u{D83D}\uDE00';
	const pair = '😀';
</script>
