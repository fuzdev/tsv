<script>
	// Expand-first-arg pattern (setTimeout) with trailing comment
	setTimeout(
		() => {
			tick();
		},
		100 // trailing comment
	);

	// Multiple function args with trailing comment
	fn(
		(a) => a,
		(b) => b // trailing comment
	);

	// Test call with trailing comment - comment moves outside (prettier quirk)
	it('test', () => {
		expect(true).toBe(true);
	}); // trailing comment

	// Test call: an own-line comment follows the relocated trailing comment
	it('test2', () => {
		expect(true).toBe(true);
	}); // trailing comment
	// own-line comment

	// Test call: a trailing block stays inside the parens
	it('test3', () => {
		expect(true).toBe(true);
	} /* b */);

	// Template literal with newlines and trailing comment
	fn(`line1
line2`); // trailing comment

	// Template literal: an own-line comment follows the relocated one
	fn(`line3
line4`); // trailing comment
	// own-line comment
</script>
