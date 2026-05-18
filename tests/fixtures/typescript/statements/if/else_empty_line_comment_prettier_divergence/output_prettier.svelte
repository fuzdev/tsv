<script>
	// Block consequent, line comment before empty alternate
	if (a) {
		expr;
	} // c
	else;

	// Non-block consequent, line comment before empty alternate
	if (a)
		expr; // c
	else;

	// Empty consequent, line comment before empty alternate
	if (a); // c
	else;
</script>
