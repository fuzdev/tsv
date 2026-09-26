<!--
	Content holding a line break, inside `<pre>`: the line break is rendered text, so the special
	element's tags break around it instead — the `>` takes the next line and the closing tag dangles
	its `>` — exactly as a component there does.
-->

<pre>a<svelte:element this="b"
		>text1
  text2</svelte:element
	>c</pre>

<pre>a<svelte:component this={Comp}
		>text1
	text2</svelte:component
	>c</pre>

<pre>{#if cond}a<svelte:self
			>text1
text2</svelte:self
		>c{/if}</pre>

<pre>a<slot
		>{expr}
  text</slot
	>c</pre>
