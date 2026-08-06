<script>
	// A comment before a leading hole's comma leads the next element
	const a = [/* c */, x];

	// Past two holes, it still leads the next element
	const b = [, /* c */ , x];

	// An own-line comment after an element leads the next element, past the hole
	const c = [x
/* c */, , y];
</script>
