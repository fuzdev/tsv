<script>
	// First call args expanded (exceed 100), last call args inline (fit)
	// Prettier keeps .c.d(x) inline when the first call already breaks
	await fn(
		a.b('/aaaaaaaaaaaaaaaaaaaaaa/bbbbbbbbbbbbbbbbbbbbb', '/ccccccccccccccc/ddddddddddddddddd'),
	).c.d(x);

	// Same pattern with longer chain
	await fn(
		a.b('/aaaaaaaaaaaaaaaaaaaaaa/bbbbbbbbbbbbbbbbbbbbb', '/ccccccccccccccc/ddddddddddddddddd'),
	).c.d.e(x);
</script>
