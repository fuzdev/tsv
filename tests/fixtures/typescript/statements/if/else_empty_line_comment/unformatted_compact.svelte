<script>
	// Block consequent, line comment before empty alternate
	if(a){expr;}else// c
	;

	// Non-block consequent, line comment before empty alternate
	if(a)expr;else// c
	;

	// Empty consequent, line comment before empty alternate
	if(a);else// c
	;
</script>
