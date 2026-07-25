<script lang="ts">
	// an own-line directive in an element gap freezes only the following element
	const a = [
		{ a:   1 }  ,
		// prettier-ignore
		{ b:   2 },
			{ c:   3 }
	];

	// before the first element
	const b = [
		// prettier-ignore
		{ a:   1 },
			{ b:   2 }
	];

	// an own-line block comment behaves identically
	const c = [
		/* prettier-ignore */
		{ a:   1 },
		{ b:     2 }
	];

	// a hole contributes only its comma, so the element after one still freezes
	const d = [
		a  ,
		,
		// prettier-ignore
		{ b:   2 }
	];

	// a lone element
	const e = [
		// prettier-ignore
		{ a:   1 }
	];

	// a spread rides inside the frozen slice
	const f = [
		// prettier-ignore
		...  a  .  b,
		  c
	];

	// a multi-line frozen element keeps its verbatim layout
	const g = [
			a,
		// prettier-ignore
		{
			b:   2,
				c: 3
		},
		d
	];

	// nested inside a call argument
	fn({
		list: [
			// prettier-ignore
			{ a:   1 },
				{ b:   2 }
		]
	});

	// a block comment glued before a frozen element is owned by it and stays put
	const i = [
		// prettier-ignore
		/* c */ { a:   1 },
		  b
	];

	// a sequence element keeps its own grouping parens around the frozen slice
	const j = [
		// prettier-ignore
		(0,   1),
		  b
	];
</script>
