<!-- A `<title>` in `<svelte:head>` preserves its content, so its head answers the `>` the way
	`<pre>` / `<textarea>` do. Attributes on a `<title>` are a compile error, so an empty
	attribute list is the only list it has, and prettier deletes every comment in one. -->
<svelte:head>
	<title /* c1 */>text1</title>

	<title
		/* c2 */>text2</title
	>

	<title // c3
		>text3</title
	>
</svelte:head>
