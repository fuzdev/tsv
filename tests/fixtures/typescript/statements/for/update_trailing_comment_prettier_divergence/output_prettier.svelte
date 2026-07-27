<script>
	// An own-line comment after the update clause keeps its own line before `)`
	for (a; b; c++)
	// c1
	{
		fn();
	}

	for (a; b; c++)
	/* c2 */
	{
		fn();
	}

	// A comment on the update's own line stays trailing it
	for (a; b; c++ /* c3 */) fn();
</script>
