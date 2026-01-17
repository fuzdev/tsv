<script>
	// Trailing comment after ) - we preserve, prettier moves inside
	while (a) // trailing
	{
		fn();
	}

	// Own-line comment before { (no blank line)
	while (a)
	// own line
	{
		fn();
	}

	// Own-line comment before { (with blank line - preserved)
	while (a)

	// blank before
	{
		fn();
	}
</script>
