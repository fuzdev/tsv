<script lang="ts">
	// Line comment after the comma stays on the first operand's line
	fn((a,     // c1
		b));

	// Block comment before the comma and line comment after — both preserved
	fn((a   /* b1 */ ,    // c1
		b));

	// A line comment in each gap of a three-operand sequence
	fn((a,  // c1
		b,  // c2
		c));

	// Own-line comment after the comma leads the next operand
	fn((a,
		// c1
		b));
</script>
