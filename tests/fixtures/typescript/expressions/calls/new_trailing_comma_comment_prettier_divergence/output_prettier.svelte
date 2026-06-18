<script>
	// block comment after the last argument's comma is preserved in place
	new Foo(
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb /* comment */,
	);

	// ...and stays after the comma when a same-line line comment trails it too
	new Bar(
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		cccccccccccccccccccccccccccccccccccccccccccccccc /* after */, // l
	);
</script>
