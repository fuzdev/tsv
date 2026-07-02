<script>
	// Single-line block glued to a unary operator with the operand on the next line stays inline.
	const a = !(/* c */ foo);

	const b = typeof (/* c */ foo);

	delete (/* c */ foo.bar);
</script>
