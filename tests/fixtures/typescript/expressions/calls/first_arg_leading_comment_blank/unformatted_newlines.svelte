<script lang="ts">

	// An own-line leading comment on the FIRST argument, with a blank line before
	// the argument: the blank is preserved (matches prettier), same as the blank
	// after a leading comment on any later argument.

	// line comment
	fn(

		// c



		a

	);

	// block comment
	fn(

		/* c */


		b

	);

	// no blank line — the comment leads the argument directly
	fn(

		// c
		c

	);

</script>
