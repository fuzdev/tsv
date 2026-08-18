<script>
	// the body tail's comments keep their lines and order at the `do`'s level
	do expr; // c1
	// c2
	// c3
	while (a);
	// a block in the `;`→`while` gap flushes behind the deferred tail, in source order
	do expr2;
	// c4
	/* c5 */
	while (b);
</script>
