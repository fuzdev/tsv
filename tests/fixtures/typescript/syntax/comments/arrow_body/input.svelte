<script lang="ts">
	// Multi-line block comment after arrow, body on separate line (#17421)
	const fn = (): any =>
		/* comment1
comment2
*/
		null;
</script>
