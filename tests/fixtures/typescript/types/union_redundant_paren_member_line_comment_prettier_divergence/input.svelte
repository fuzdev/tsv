<script lang="ts">
	// a redundant parenthesized union member - its parens are stripped, so a leading
	// line comment inside them cannot stay "inside"; tsv leads the member with it on its
	// own line before the `| ` (prettier trails it on the previous member, variant_trailing)
	type Mid =
		| a
		// c
		| b
		| d;

	// nested redundant parens collapse the same way, comment still leading the member
	type Nested =
		| p
		// c
		| q
		| r;
</script>
