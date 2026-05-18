<script lang="ts">
	// Inline block comment after comma (leading on next element)
	type A = [string, /* c */ number];

	// Inline block comment before comma (trailing on current element)
	type B = [string /* c */, number];

	// Both trailing and leading comments around comma
	type C = [string /* c1 */, /* c2 */ number];
</script>
