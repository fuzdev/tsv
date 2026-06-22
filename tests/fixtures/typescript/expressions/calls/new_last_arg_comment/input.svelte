<script>
	// block comment trailing the last argument is preserved in place
	new Foo(
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb /* comment */
	);

	// ...and stays put when a same-line line comment trails it too
	new Bar(
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		cccccccccccccccccccccccccccccccccccccccccccccccc /* after */ // l
	);
</script>
