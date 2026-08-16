<script>
	// a line comment before the lone argument, and a block trailing the argument
	f(
		// c
		a /* t */
	);

	// two trailing blocks
	f(
		// c
		a /* t1 */ /* t2 */
	);

	// an arrow argument
	f(
		// c
		(x) => x /* t */
	);

	// `new` and a member callee reach the same gap through their own printers
	new F(
		// c
		a /* t */
	);
	obj.m(
		// c
		a /* t */
	);
</script>
