<script>
	// the array family measures a blank line from past the comma, so a blank the author left
	// after a comma of its own is authorship and survives; the trailing line comment is what
	// expands the pattern, which a blank line alone does not
	const [a1,

	b1
	// c1
	] = x1;

	// a line break around the comma with no blank after it is layout, not authorship
	const [a2

	,
	b2
	// c2
	] = x2;
</script>
