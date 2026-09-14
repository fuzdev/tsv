<!-- a line comment after the comma trails it; the identifiers below hang one level in and
	the `}` drops to the tag's column, as at every braced head a `//` breaks -->
{@debug a, // c1
	b
}

<!-- an own-line comment after the comma keeps its line, leading the next identifier -->
{@debug a,
	// c2
	b
}

<!-- an author blank below it survives -->
{@debug a,
	// c3

	b
}

<!-- a run: what shares the comma's line trails it, the rest leads the identifier -->
{@debug a, // c4
	// c5
	b, c
}
