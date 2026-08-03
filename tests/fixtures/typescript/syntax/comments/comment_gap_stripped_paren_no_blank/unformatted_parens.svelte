<script lang="ts">
	// declarator init: line comment, gap spans the stripped paren
	const a =
	// c1
	(
		x
	);

	// declarator init: block comment
	const b =
	/* c1 */
	(
		x
	);

	// a run of two comments
	const c =
	// c1
	// c2
	(
		x
	);

	// array element
	const d = [
		x,
		/* c1 */
		(
			y
		)
	];

	// call argument
	fn(
		x,
		// c1
		(
			y
		)
	);

	// class property
	class K {
		p =
		// c1
		(
			x
		);
	}

	// nested parens
	const e =
	// c1
	(
		(
			x
		)
	);
</script>
