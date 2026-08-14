<script>
	// Switch - a glued pair below the header keeps the one line the author gave it
	switch (a)
	/* b1 */ /* b2 */
	{
		case 1:
			break;
	}

	// Switch - a run ending in a line comment stays glued and still breaks
	switch (a)
	/* b1 */ /* b2 */ // b3
	{
		case 1:
			break;
	}

	// Switch - control, comments the author gave separate lines keep them
	switch (a)
	/* b1 */
	/* b2 */
	{
		case 1:
			break;
	}
</script>
