<script>
	// for: line comment between header and non-block body
	for (;;)
		// c1
		expr;

	// for: block comment (already correct)
	for (;;) /* c1 */ expr;

	// do: line comment between do and non-block body
	do
		// c1
		expr;
	while (a);

	// do: block comment (already correct)
	do /* c1 */ expr;
	while (a);
</script>
