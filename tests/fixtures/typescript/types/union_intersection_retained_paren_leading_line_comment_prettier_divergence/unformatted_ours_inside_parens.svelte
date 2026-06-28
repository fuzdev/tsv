<script lang="ts">
	// first union member is a parenthesized union with a leading line comment inside
	// the parens - tsv keeps the comment inside the member (prettier moves it out, before
	// the member); the comment takes its own line inside the parens, above the inline union
	type First =
		| (// c
		A | B)
		| C;

	// a leading line comment inside a LATER member's parens relocates to trail the
	// previous member (both formatters agree); only the first member - with no previous
	// member to relocate onto - keeps the comment inside the parens
	type Mid =
		| A
		| (// c
		B | C)
		| D;
</script>
