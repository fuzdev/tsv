<script>
	// a block followed by another statement is one statement of the list: it takes the
	// consequent's own indented line, as the statement after it does
	switch (x) {
		case 'a':
			{
				fn1();
			}
			break;
		default:
			{
				fn1();
			}
			break;
	}

	// any number of statements may follow, an author blank line between them kept
	switch (x) {
		case 'a':
			{
				fn1();
			}

			fn2();
			break;
	}

	// an empty block, and a block followed by another block
	switch (x) {
		case 'a':
			{
			}
			break;
		case 'b':
			{
				fn1();
			}
			{
				fn2();
			}
	}

	// comments around the block keep their places
	switch (x) {
		case 'a': // c1
			{
				fn1();
			} // c2
			// c3
			break;
	}

	// a block comment glued to the `{` goes with the block, as one glued to any other first
	// statement does
	switch (x) {
		case 'a':
			/* c4 */ {
				fn1();
			}
			break;
	}

	// a nested switch answers the same way at its own depth
	switch (x) {
		case 'a':
			{
				switch (y) {
					case 'b':
						{
							fn1();
						}
						break;
				}
			}
			break;
	}

	// null control: a block that is its case's only statement keeps the label line, beside a
	// case whose block is not
	switch (x) {
		case 'a':
			{
				fn1();
			}
			break;
		case 'b': {
			fn1();
		}
	}
</script>
