<script lang="ts">
	// comment inside the angle brackets, before the type
	const a = </* c */ string>x;
	// comment after the type, before the closing angle bracket
	const b = <string /* c */>y;
	// comment after the closing angle bracket, before the expression
	const c = <string>/* c */ z;
	// a glued run the author broke after, holding a multi-line block: the type drops below
	const d = <( /* c1
	c2 */ /* c3 */
	string)>w;
</script>
