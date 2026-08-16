<script lang="ts">
	// the pattern's closing bracket is found past a comment holding one, so a
	// comment in the bracket→`:` gap stays outside the pattern either way
	const { a /* } */ } /* c1 */ : T = x;
	const [b /* ] */] /* c2 */ : U = y;

	// a string default holding one does not end the pattern either
	const { c = '}' } /* c3 */ : V = z;
</script>
