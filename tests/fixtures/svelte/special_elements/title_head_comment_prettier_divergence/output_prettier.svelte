<!-- A `<title>` in `<svelte:head>` preserves its content, so its head answers the `>` the way
	`<pre>` / `<textarea>` do. Attributes on a `<title>` are a compile error, so an empty
	attribute list is the only list it has, and prettier deletes every comment in one. -->
<svelte:head>
	<title>text1</title>

	<title>text2</title>

	<title>text3</title>
</svelte:head>
