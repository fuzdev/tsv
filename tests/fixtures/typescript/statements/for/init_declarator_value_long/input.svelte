<script lang="ts">
	// A `for` header's own declarator is a declarator: its `=` takes the same assignment
	// layout the statement-level one takes, so an over-width initializer breaks after the
	// `=` and its continuation is indented one level.

	// 100 - the declarator fits on the clause line
	for (
		let aa1 = bbbbbbb + ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc;
		aa1 < nn;
		aa1++
	)
		gg();

	// 101 - one char longer: the value drops below the `=`
	for (
		let aa2 =
			bbbbbbb + dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd;
		aa2 < nn;
		aa2++
	)
		gg();

	// the value's own operators break under that same indent when the continuation overflows
	for (
		let aa3 =
			bbbbbbb +
			eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee;
		aa3 < nn;
		aa3++
	)
		gg();

	// a call initializer expands its arguments rather than breaking after the `=`
	for (
		let aa4 = fnnnnn(
			ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff
		);
		aa4 < nn;
		aa4++
	)
		gg();

	// the call-object clause holds here too: a lone `.prop` off a call WITH ARGUMENTS
	// carries no break point, so the width sheds into the arguments
	for (
		let aa5 =
			fnnnnn(
				ggg
			).hhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhh;
		aa5 < nn;
		aa5++
	)
		gg();

	// NOT the clause: the call has no arguments, so the lookup keeps its break point
	for (
		let aa6 =
			fnnnnn()
				.iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii;
		aa6 < nn;
		aa6++
	)
		gg();

	// NOT the clause: two lookups, so the last one's object is a member and not the call
	for (
		let aa7 =
			fnnnnn(ggg).jj
				.kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk;
		aa7 < nn;
		aa7++
	)
		gg();
</script>
