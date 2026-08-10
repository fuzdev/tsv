<script lang="ts">
	// A glued pair of block comments takes a soft line before the argument, so a list that
	// fits keeps the pair on the argument's line.
	fn1(/* c1 */ /* c2 */ a);

	// The same between arguments.
	fn2(a, /* c1 */ /* c2 */ b);

	// `new` takes its own argument printer.
	new Comp(a, /* c1 */ /* c2 */ b);

	// So does a member chain long enough to break.
	obj.m1().m2(a, /* c1 */ /* c2 */ b);

	// A block the author isolated on its own line takes a hardline instead, so it forces
	// the list open.
	fn3(
		a,
		/* c */
		b
	);

	// The soft line breaks with the list, so a list too wide to fit keeps the pair on its
	// own line.
	fn4(
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong,
		/* c1 */ /* c2 */
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbLong
	);

	// A lone argument in a member chain takes yet another argument printer - the pair
	// stays on its line there too.
	obj.m3().m4(/* c1 */ /* c2 */ a);

	// And drops below the pair when the chain breaks.
	obj
		.m5()
		.m6(
			/* c1 */ /* c2 */
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong
		);
</script>
