<script>
	// a rest element's stripped parens leave an own-line comment behind; a comment written
	// after the `)` follows it, in the order the author wrote them — in an array and in an
	// object assignment pattern

	// a block after the `)` glues onto the inside comment's line
	[...(a
	/* i */) /* t */] = x;

	({...(a
	/* i */) /* t */} = x);

	[b, ...(a
	/* i */) /* t */] = x;

	({b, ...(a
	/* i */) /* t */} = x);

	// a multi-line block inside the parens keeps its lines
	({...(a
	/* i1
	i2 */) /* t */} = x);

	// a line comment inside the parens ends its line, so the comment after the `)` starts
	// the next one
	[b, ...(a
	// i
	) /* t */] = x;

	({b, ...(a
	// i
	) /* t */} = x);

	[b, ...(a
	// i
	) // t
	] = x;

	({b, ...(a
	// i
	) // t
	} = x);

	// a line comment on the rest's line inside the parens ends that line too
	[...(a // i
	) /* t */] = x;

	({...(a // i
	) /* t */} = x);

	// a nested pattern, and a for-of head
	[b, [c, ...(a
	// i
	) /* t */]] = x;

	for ([b, ...(a
	/* i */) /* t */] of xs);

	// unchanged: an own-line block after the `)` of a line comment's share
	[b, ...(a
	// i
	)
	/* t */] = x;

	({b, ...(a
	// i
	)
	/* t */} = x);

	[b, ...(a // i
	)
	/* t */] = x;
</script>
