<script>
	// Block comment between ) and body (stays inline)
	for (x in obj) /* a */ expr;

	for (x of arr) /* a */ expr;

	// Line comment between ) and body (indents body)
	for (x in obj) // a
		expr;

	for (x of arr) // a
		expr;

	// Line comment + block comment between ) and body
	for (x in obj) // a
		/* b */ expr;

	for (x of arr) // a
		/* b */ expr;
</script>
