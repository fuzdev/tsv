<script lang="ts">
	// keyof operator, trailing line comment: comment after the keyword, operand on the next line
	type A = keyof ((// c
		B));

	// typeof operator (a TypeQuery), trailing line comment
	type B = typeof // c
		x;

	// keyof operator, own-line line comment: comment on its own line, operand on the next line
	type C = keyof
		// c
		B;

	// typeof operator, own-line line comment
	type D = typeof
		// c
		x;
</script>
