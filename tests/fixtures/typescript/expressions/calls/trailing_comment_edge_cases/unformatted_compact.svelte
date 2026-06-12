<script>
	// Expand-first-arg pattern (setTimeout) with trailing comment
	setTimeout(()=>{tick();},100, // trailing comment
);

	// Multiple function args with trailing comment
	fn((a)=>a,(b)=>b, // trailing comment
);

	// Test call with trailing comment - comment moves outside (prettier quirk)
	it('test',()=>{expect(true).toBe(true);}, // trailing comment
);

	// Template literal with newlines and trailing comment
	fn(`line1
line2`, // trailing comment
);
</script>
