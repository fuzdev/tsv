<script>
	// a same-line comment left behind by an object spread's stripped parens rides the
	// property's line — printed by the spread's own doc, never again by the object
	const o1 = { ...(b /* i */), c };

	const o2 = { c, ...(b /* i */) };

	// a line comment inside the stripped parens runs to end of line, so the object
	// breaks around it
	const o3 = {...(b // i
	), c};

	const o4 = {a, ...(b // i
	), c};

	const o5 = {...(b // i
	)};

	// an own-line comment is the object's share instead: a sibling line ahead of the
	// next property
	const o6 = {...(b
	/* i */
	), c};

	// an author blank line after the spread survives
	const o7 = {...(b // i
	),

	c};
</script>
