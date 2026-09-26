<!--
	Inside `<pre>` a special element's head takes the whitespace-sensitive family's shape around a
	`//`: a list ending on one puts the `>` on the next line, one level in, and the closing tag
	dangles its `>`.
-->

<!-- a `//` inside the `this` binding: the list wraps and the `>` hugs the `}` -->
<pre>a<svelte:element this={tag}>text1   text2</svelte:element>c</pre>

<!-- a list ending on a `//` -->
<pre>a<svelte:element
		this="b"
		data-attr="value">text1   text2</svelte:element // c
	>c</pre>

<pre>a<svelte:component
		this={Comp}
		prop="value">text1   text2</svelte:component // c
	>c</pre>
