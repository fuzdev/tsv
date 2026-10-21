<script lang="ts">
	// a comment between the sole parameter and its `)` declines the hug, so the authored
	// expansion of its object type breaks the call's arguments out
	fn((a: {
		b: C;
	} /* c */) => {
		g(a);
	});
</script>
