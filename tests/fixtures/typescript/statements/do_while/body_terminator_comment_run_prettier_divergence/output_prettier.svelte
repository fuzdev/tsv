<script>
	// the body tail's comments keep their lines and order at the `do`'s level
	do
		expr; // c1
	while (
		// c2
		// c3
		a
	);
	// a block in the `;`→`while` gap flushes behind the deferred tail, in source order
	do expr2;
	while (
		// c4
		/* c5 */
		b
	);
</script>
