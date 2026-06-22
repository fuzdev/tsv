<script>
	// Regular function call - works correctly
	fn(
		arg1, // comment
		arg2
	);

	// Constructor call - bug: trailing comment is stripped
	new Class(
		arg1, // comment
		arg2
	);

	// Multiple trailing comments in constructor
	new Class(
		arg1, // comment 1
		arg2 // comment 2
	);
</script>
