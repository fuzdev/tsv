<script lang="ts">
	// Inline block comment after comma (leading on next type argument)
	type A = Map<string, /* c */ number>;

	// Inline block comment before comma (trailing on current type argument)
	type B = Map<string /* c */, number>;

	// Both trailing and leading comments around comma
	type C = Map<string /* c1 */, /* c2 */ number>;
</script>
