<script lang="ts">
	// A lone surrogate is well-formed grammar in both escape forms — only the
	// well-formed-unicode proposals care. The 4-digit form already parses
	const lead4 = '\uD800';
	const trail4 = '\uDC00';

	// The `\u{}` form takes the same code points
	const lead = '\u{D800}';
	const trail = '\u{DC00}';
	const template = `\u{D800}`;

	// A well-formed PAIR is unaffected
	const pair = '😀';
</script>
