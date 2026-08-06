<script>
	// an own-line comment left behind by a spread's stripped parens, on an argument the
	// list continues past: it becomes a sibling line ahead of the next argument
	fn(...(b
	/* i */
	), c);

	new A(...(b
	/* i */
	), c);

	obj.method(...(b
	/* i */
	), c);

	// the same on an argument in the middle of the list
	fn(a, ...(b
	/* i */
	), c);

	// a block written after the stripped `)` sits before the comma, so it trails the
	// argument; the own-line comment keeps the line below
	fn(...(b
	/* i */
	) /* t */, c);

	// a line comment written on its own line after the `)` keeps its own line too
	fn(...(b
	/* i */
	)
	// t
	, c);
</script>
