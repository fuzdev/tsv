<script>
	// Block comment between left and keyword (stays inline)
	for (x/* a */in obj) expr;

	for (x/* a */of arr) expr;

	// Block comment between keyword and right (stays inline)
	for (x in/* a */obj) expr;

	for (x of/* a */arr) expr;

	// Line comment between left and keyword (breaks)
	for (x// a
	in obj) expr;

	for (x// a
	of arr) expr;

	// Line comment between keyword and right (breaks)
	for (x in// a
	obj) expr;

	for (x of// a
	arr) expr;

	// Block comment between ) and body in breaking layout
	for (x// a
	in obj)/* b */expr;

	for (x// a
	of arr)/* b */expr;

	// With declaration
	for (const a/* a */in obj) expr;

	for (const a/* a */of arr) expr;
</script>
