<script lang="ts">
	// a comment in an argument gap gives up the test call's flat one-line layout, so the
	// callback's parameter list breaks on width like any other call's - 100 stays flat
	it(
		// c
		'text',
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: A, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb: B) => {
			a();
		}
	);

	// 101 - the list breaks, one parameter per line
	it(
		// c
		'text',
		(
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: A,
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb: B
		) => {
			a();
		}
	);

	// control: with no gap comment the same 101-wide list keeps the flat layout, where a
	// test call's parameters stay on one line at any width
	it('text', (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: A, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb: B) => {
		a();
	});

	// type parameters take the same rule - 100 stays flat
	it(
		// c
		'text',
		<TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT, UUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUU>() => {
			a();
		}
	);

	// 101 - the type-parameter list breaks
	it(
		// c
		'text',
		<
			TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,
			UUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUU
		>() => {
			a();
		}
	);
</script>
