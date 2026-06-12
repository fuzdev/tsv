<script lang="ts">
	// Arrow with block comment - prettier moves inside parens, we preserve between > and (
	const a = <T,> /* c */(x: T) => x;
</script>
