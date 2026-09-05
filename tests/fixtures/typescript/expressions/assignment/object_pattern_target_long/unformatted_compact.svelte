<script>
	// Object-pattern target with a logical value - fits inline (100 chars)
	({ aaaaaaaaaa } = xxxxxxxxxxxxxxxxxxxxxxxx ?? yyyyyyyyyyyyyyyyyyyyyyyy ?? zzzzzzzzzzzzzzzzzzzzzz);

	// Object-pattern target with a logical value - breaks after the operator (101 chars)
	({ aaaaaaaaaa } = xxxxxxxxxxxxxxxxxxxxxxxx ?? yyyyyyyyyyyyyyyyyyyyyyyy ?? zzzzzzzzzzzzzzzzzzzzzzz);

	// The value's own operands break flush on the continuation line
	({ aaaaaaaaaa } = xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx ?? yyyyyyyyyyyyyyyyyyyyyyyyyyyyyy ?? zzzzzzzzzzzzzzzzzzzzzzzzzzzzzz);

	// A member chain and a string value break after the operator the same way
	({ bbbbbbbbbb } = cccccccccccccccccccccccccccccc.dddddddddddddddddddddddddddd.eeeeeeeeeeeeeeeeeeeeeeeeee);
	({ length } = 'a string literal that is long enough not to fit on the statement line at all on its own');
</script>
