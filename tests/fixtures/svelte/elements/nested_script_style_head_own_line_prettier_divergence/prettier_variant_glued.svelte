<!--
	Inside `<svelte:head>` the `<title>` is hoisted and the head's content joins no line of the
	page, so a nested `<script>` after it takes its own line.
-->

<svelte:head
	><title>text</title><script type="application/ld+json">
		{ "a": 1 }
	</script><meta name="value1" content="value2" /></svelte:head
>
