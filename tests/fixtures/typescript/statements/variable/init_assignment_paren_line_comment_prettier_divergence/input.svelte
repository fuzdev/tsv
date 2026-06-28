<script lang="ts">
	// A line comment trailing a parenthesized assignment operand in a value position
	// stays INSIDE the parens, like the sequence and block cases; prettier floats it
	// out past the `;`.
	const y = (
		a = b // c
	);
</script>
