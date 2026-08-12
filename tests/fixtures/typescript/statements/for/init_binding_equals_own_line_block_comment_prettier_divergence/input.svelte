<script>
	// An own-line block comment collapses to trail the binding.
	for (let g /* c */ = 0; g < 10; g++) {}

	// A run collapses in order, each comment distinct.
	for (let h /* c1 */ /* c2 */ = 0; h < 10; h++) {}
</script>
