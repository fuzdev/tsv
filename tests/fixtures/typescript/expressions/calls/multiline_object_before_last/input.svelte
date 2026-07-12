<script lang="ts">
	// A source-multiline object among the leading (non-last) args breaks across
	// lines, so all args go onto their own lines — the trailing callback or object
	// does not hug the call parens.

	// multiline object before a callback (2 args) — all args break
	fn(
		{
			aaaa: 1,
			bbbb: 2
		},
		() => {
			doThing();
		}
	);

	// inline object before a callback (2 args) — expand-last still hugs the callback
	fn({ aaaa: 1, bbbb: 2 }, () => {
		doThing();
	});

	// multiline object before a middle arg and a trailing object (3 args) — all break
	fn(
		{
			aaaa: 1,
			bbbb: 2
		},
		middle,
		{ cccc: 3 }
	);
</script>
