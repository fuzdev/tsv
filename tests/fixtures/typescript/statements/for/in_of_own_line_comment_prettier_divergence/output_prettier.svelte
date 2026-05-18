<script>
	// Block comment between left and keyword (stays inline)
	for (x /* a */ in obj) expr;

	for (x /* a */ of arr) expr;

	// Block comment between keyword and right (stays inline)
	for (x in /* a */ obj) expr;

	for (x of /* a */ arr) expr;

	// Line comment between left and keyword (breaks)
	for (x in obj) // a
		expr;

	for (x of arr) // a
		expr;

	// Line comment between keyword and right (breaks)
	for (x in obj) // a
		expr;

	for (x of arr) // a
		expr;

	// Block comment between ) and body in breaking layout
	for (x in obj) // a
		/* b */ expr;

	for (x of arr) // a
		/* b */ expr;

	// With declaration
	for (const a /* a */ in obj) expr;

	for (const a /* a */ of arr) expr;
</script>
