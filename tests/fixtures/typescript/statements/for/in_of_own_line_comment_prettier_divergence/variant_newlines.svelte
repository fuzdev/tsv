<script>
	// Block comment between left and keyword (stays inline)
	/* a */
	for (x in obj) expr;

	/* a */
	for (x of arr) expr;

	// Block comment between keyword and right (stays inline)
	/* a */
	for (x in obj) expr;

	/* a */
	for (x of arr) expr;

	// Line comment between left and keyword (breaks)
	// a
	for (x in obj) expr;

	// a
	for (x of arr) expr;

	// Line comment between keyword and right (breaks)
	// a
	for (x in obj) expr;

	// a
	for (x of arr) expr;

	// Block comment between ) and body in breaking layout
	// a
	for (x in obj)
		/* b */
		expr;

	// a
	for (x of arr)
		/* b */
		expr;

	// With declaration
	/* a */
	for (const a in obj) expr;

	/* a */
	for (const a of arr) expr;
</script>
