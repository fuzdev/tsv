<script>
	// Trailing line comment after a binary argument in a `new` call - binary stays inline
	new Class(
		aaaaaaaaaa === bbbbbbbbbb, // comment that pushes the new argument line over the print width xxxx
		cccc,
	);
</script>
