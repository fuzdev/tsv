<script lang="ts">
	// a comment written INSIDE a member's redundant paren shell is that member's own
	// comment, so the union declines the hug and prints one member per line

	// the object member's shell, trailing gap
	type Trail = G<({
		a: 1
	} /* c */) | null>;

	// a line comment in the object member's shell, leading gap
	type LeadLine = G<(// c
	{
		a: 1
	}) | null>;

	// the void sibling's shell, leading gap
	type Sibling = G<{
		a: 1
	} | (/* c */ null)>;

	// a comment NESTED inside the member is the member's own doc's, so the hug stays
	type Inner = G<({
		/* c */
		a: 1
	}) | null>;
</script>
