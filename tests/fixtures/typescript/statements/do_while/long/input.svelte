<script>
	// 100 chars exactly - stays inline (at boundary)
	do {
		a();
	} while (condA && condB && condC && condD && condE && condF && condG && condHHHHHHHHHHHHHHHHHHHH);

	// 101 chars - the condition parens open and the operands indent
	do {
		a();
	} while (
		condA &&
		condB &&
		condC &&
		condD &&
		condE &&
		condF &&
		condG &&
		condHHHHHHHHHHHHHHHHHHHHH
	);

	// Negated parenthesized logical hugs the `(` rather than breaking onto its own line
	do {
		a();
	} while (!(
		condA ||
		condB ||
		condC ||
		condD ||
		condE ||
		condF ||
		condG ||
		condHHHHHHHHHHHHHHHHHH
	));

	// Non-block body keeps `while` on its own line
	do a();
	while (
		condA &&
		condB &&
		condC &&
		condD &&
		condE &&
		condF &&
		condG &&
		condHHHHHHHHHHHHHHHHHHHHHHHHHH
	);
</script>
