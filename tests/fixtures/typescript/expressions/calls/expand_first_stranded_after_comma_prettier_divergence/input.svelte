<script>
	// a block the author stranded after the first argument's comma trails that
	// argument, so the expand-first hug is declined (prettier's `!hasComment(firstArg)`)
	// and every argument breaks out; the block keeps the comma line it was written on
	fn(
		() => {
			a();
		}, /* c */
		b
	);

	new A(
		() => {
			a();
		}, /* c */
		b
	);

	obj.fn1(1).fn2(
		() => {
			a();
		}, /* c */
		b
	);

	// the before-comma spelling of the same refusal, where both formatters agree on
	// the block's position
	fn(
		() => {
			a();
		} /* c */,
		b
	);
</script>
