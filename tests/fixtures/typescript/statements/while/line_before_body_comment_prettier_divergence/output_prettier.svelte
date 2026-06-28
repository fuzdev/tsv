<script>
	// Trailing comment after ) - preserved, prettier absorbs into block
	while (a) // trailing
	{
		fn();
	}

	// Own-line comment before { - preserved on own line
	while (a)
	// own line
	{
		fn();
	}

	// Blank line before comment - blank line preserved
	while (a)
	// blank before
	{
		fn();
	}
</script>
