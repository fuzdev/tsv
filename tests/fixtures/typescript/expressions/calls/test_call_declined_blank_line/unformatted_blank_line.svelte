<script lang="ts">
	// a test call that gives up its flat layout for an argument-gap comment still reads no
	// blank line between its arguments, so it hugs its callback as prettier's test call does
	it('name' /* c */,

	() => {
		g();
	});

	// an ordinary call keeps the author's blank line and breaks its arguments out
	foo(
		'name' /* c */,

		() => {
			g();
		}
	);
</script>
