<script>
	// Line comment on its own line between ) and the switch body brace
	switch (a)
	// c1
	{
		case 1:
			break;
	}

	// Line comment trailing ) stays trailing, brace drops to the next line
	switch (a) // c2
	{
		case 1:
			break;
	}

	// Blank between two own-line comments is preserved
	switch (a)
	// c3

	// c4
	{
		case 1:
			break;
	}

	// Block comment authored trailing ) stays trailing, brace on the same line
	switch (a) /* c5 */ {
		case 1:
			break;
	}

	// Block comment authored on its own line keeps it, brace drops below
	switch (a)
	/* c6 */
	{
		case 1:
			break;
	}
</script>
