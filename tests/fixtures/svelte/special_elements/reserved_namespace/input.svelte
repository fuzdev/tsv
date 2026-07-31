<!--
	The `svelte:` namespace is reserved: a tag in it must name one of the known meta
	tags, so an unknown local name is a parse error (see the input_invalid_* files).
	Only that exact prefix is reserved — every other namespace stays an ordinary
	namespaced element.
-->
<svelte:head>
	<title>text1</title>
</svelte:head>

<sveltex:foo>text2</sveltex:foo>
<foo:bar>text3</foo:bar>
