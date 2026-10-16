<script lang="ts">
	// A `<<` opens a type-argument region and a later sibling's `>>` closes it
	fn(a << b, c >> (d, e));
	const a1 = [a << b, c >> (d, e)];
</script>
