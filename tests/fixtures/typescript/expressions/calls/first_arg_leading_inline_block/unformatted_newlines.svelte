<script lang="ts">

	// An inline block hugging the first argument stays on the argument's line,
	// even when an own-line comment precedes it (both formatters keep the hug).

	// own-line line comment, then a hugging block
	fn(

		// c
		/* b */ a

	);

	// two hugging blocks after the line comment
	fn(

		// c
		/* b1 */ /* b2 */ d

	);

	// multi-arg: own-line line comment then a hugging block on the first arg
	fn(

		// c
		/* b */ e,
		g

	);

	// multi-arg: own-line block comment then a hugging block on the first arg
	fn(

		/* o */
		/* b */ h,
		i

	);

</script>
