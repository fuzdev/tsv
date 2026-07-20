<script>
	// If - two block comments before an empty body
	if (a) /* b1 */ /* b2 */ ;

	// While - two block comments before an empty body
	while (a) /* b1 */ /* b2 */ ;

	// For - two block comments before an empty body
	for (;;) /* b1 */ /* b2 */ ;

	// For-of - two block comments before an empty body
	for (x of y) /* b1 */ /* b2 */ ;

	// If - three block comments before an empty body
	if (a) /* b1 */ /* b2 */ /* b3 */ ;

	// If - single block comment before an empty body (control)
	if (a) /* b1 */ ;
</script>
