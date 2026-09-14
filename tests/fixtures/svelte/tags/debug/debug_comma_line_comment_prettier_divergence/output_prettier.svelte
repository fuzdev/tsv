<!-- a line comment after the comma trails it; the identifiers below hang one level in and
	the `}` drops to the tag's column, as at every braced head a `//` breaks -->
{@debug a, b}

<!-- an own-line comment after the comma keeps its line, leading the next identifier -->
{@debug a, b}

<!-- an author blank below it survives -->
{@debug a, b}

<!-- a run: what shares the comma's line trails it, the rest leads the identifier -->
{@debug a, b, c}
