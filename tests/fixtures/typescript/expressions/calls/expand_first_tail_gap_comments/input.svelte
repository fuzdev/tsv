<script>
	// an own-line comment in the first gap needs a line of its own, which the inline
	// tail cannot give it, so every argument breaks out
	fn(
		() => {
			a();
		},
		/* c */
		b
	);

	new A(
		() => {
			a();
		},
		/* c */
		b
	);

	obj.fn1(1).fn2(
		() => {
			a();
		},
		/* c */
		b
	);

	// a block trailing the tail argument rides the same inline tail the argument does,
	// so the expand-first hug survives and both comments are kept
	fn(() => {
		a();
	}, /* c1 */ b /* c2 */);

	new A(() => {
		a();
	}, /* c1 */ b /* c2 */);

	obj.fn1(1).fn2(() => {
		a();
	}, /* c1 */ b /* c2 */);
</script>
