<script lang="ts">
	// a test call's flat one-line layout has no argument-gap comment emitter, so a
	// comment in the leading or an inter-argument gap gives up that layout and the
	// call expands like any other — tsv keeps the comment on its own line where
	// prettier relocates it onto the `(` (or the previous argument's) line
	it(// c
	'text', () => {
		a();
	});

	it('text', // c
	() => {
		a();
	});

	// the block spelling behaves the same
	describe(/* c */
	'text', () => {
		a();
	});

	// a directive alone on its line freezes the following argument here too
	test(// prettier-ignore
	'te  xt', () => {
		a();
	});

	// a glued block comment is owned by its argument and rides inside its doc, so the
	// flat layout survives
	it('text', /* c */ () => {
		a();
	});

	// a trailing comment after the last argument is emitted by the flat layout itself
	it('text', () => {
		a();
	}); // c
</script>
