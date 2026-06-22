<script lang="ts">
	// A call with a multiline-content argument routes through the shared joined-args
	// path. A non-last argument with a block comment after the comma and a line
	// comment after it keeps the block on the comma line, the line comment trailing.
	fn(
		a, /* c1 */ // c2
		`line1
line2`
	);
</script>
