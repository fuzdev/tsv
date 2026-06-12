<script>
	// If - block comment before block body
	if (a) /* comment */ {
	}

	// If - block comment before non-block body
	if (a) /* comment */ fn();

	// If - line comment before non-block body
	if (a)
		// comment
		fn();

	// If - own-line line comment before block body
	if (a) // comment
	{
	}

	// If - multiple block comments before block body
	if (a) /* first */ /* second */ {
	}

	// If - block + line mixed before block body
	if (a) /* block */ // line
	{
	}

	// If - own-line block normalizes to inline
	if (a) /* own line block */ {
	}

	// While - block comment before non-block body
	while (a) /* comment */ fn();

	// While - line comment before non-block body
	while (a)
		// comment
		fn();

	// For-in - block comment before block body
	for (a in b) /* comment */ {
	}

	// For-of - block comment before block body
	for (a of b) /* comment */ {
	}

	// Do - block comment between do and body
	do /* comment */ {} while (a);

	// Else - block comment before block body
	if (a) {
	} else /* comment */ {
	}

	// Else - block comment before non-block body
	if (a) {
	} else /* comment */ fn();

	// Try - block comment before body
	try /* comment */ {
	} catch (e) {}

	// Catch param - block comment before body
	try {
	} catch (e) /* comment */ {}

	// Catch no param - block comment before body
	try {
	} catch /* comment */ {}

	// Finally - block comment before body
	try {
	} finally /* comment */ {
	}

	// Else - long block comment (exceeds short lookback)
	if (a) {
	} else /* very long block comment that exceeds twenty bytes easily */ {
	}
</script>
