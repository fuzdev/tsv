<script lang="ts">
	// comment inside the angle brackets, before the type
	const a = </* c */string>x;
	// comment after the type, before the closing angle bracket
	const b = <string/* c */>y;
	// comment after the closing angle bracket, before the expression
	const c = <string>/* c */z;
</script>
