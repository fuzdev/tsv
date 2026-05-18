<script>
	// Block consequent, empty alternate
	if(a){expr;}else;

	// Non-block consequent, empty alternate
	if(a)expr;else;

	// Chained: block, then empty alternate
	if(a){expr;}else if(b){expr;}else;

	// Chained: non-block, then empty alternate
	if(a)expr;else if(b)expr;else;
</script>
