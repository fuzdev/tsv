<script>
	// A block comment after the element's comma leads the next element, past the hole
	const a = [x, /* c */ , y];

	// The run keeps its authored order
	const b = [x, /* c1 */ /* c2 */ , y];

	// Past several holes it still leads the next element
	const c = [x, , /* c */ , , y];
</script>
