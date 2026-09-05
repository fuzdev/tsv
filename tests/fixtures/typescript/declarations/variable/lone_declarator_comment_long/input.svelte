<script>
	// A comment past the keyword belongs to the DECLARATOR, so a lone declarator carrying
	// one takes the first-declarator indent and every breaking value hangs two levels
	const /* c */ a =
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb === cccccccccccccccccccccccccccccccccccc;

	// The indent wraps the whole declarator, so a call's arguments hang two levels too
	const /* c */ b = fn(
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
			cccccccccccccccccccccccccccccccccccc
		);

	// And an object literal, which breaks on its own rather than after `=`
	const /* c */ cc = {
			x: 1
		};

	// A comment between the name and `=` belongs to a child node - one level
	const d /* c */ =
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb === cccccccccccccccccccccccccccccccccccc;

	// No comment at all - one level
	const e =
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb === cccccccccccccccccccccccccccccccccccccccccccc;

	// A list already takes the indent, comment or not
	let /* c */ f =
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb === cccccccccccccccccccccccccccccccccccccccc,
		g = 1;

	// 100 chars - stays inline
	const /* c */ h = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb === cccccccccccccccccccccccccccccccccc;

	// 101 chars - breaks after `=` and hangs two levels
	const /* c */ i =
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb === ccccccccccccccccccccccccccccccccccc;
</script>
