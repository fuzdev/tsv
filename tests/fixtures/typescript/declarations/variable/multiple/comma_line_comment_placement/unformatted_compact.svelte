<script>
	// Same-line line comment after the comma trails it.
	let a1=1, // line
	b1=2;

	// Own-line line comment after the comma sits on its own line below.
	let a2=1,
	// line
	b2=2;

	// Trailing block before the comma plus a same-line line comment: both trail the comma.
	let a3=1 /* block */, // line
	b3=2;

	// Trailing block before the comma plus an own-line line comment: the line comment sits below.
	let a4=1 /* block */,
	// line
	b4=2;
</script>
