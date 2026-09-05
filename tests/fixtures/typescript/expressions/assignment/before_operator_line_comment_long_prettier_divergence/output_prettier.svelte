<script>
	// The operator and value take a continuation line; the value fits on it (100 chars).
	a = xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx ?? yyyyyyyyyyyyyyyyyyyyyyyyyyyyyy ?? zzzzzzzzzzzzzzzzzzzzzzzzz; // c1

	// The same continuation with a value that does not fit - the logical operands break
	// flush with the operator (101 chars).
	b = // c2
		xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx ?? yyyyyyyyyyyyyyyyyyyyyyyyyyyyyy ?? zzzzzzzzzzzzzzzzzzzzzzzzzz;
</script>
