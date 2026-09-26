<!--
	Inside `<pre>` a block among a special element's content is laid out like the rest of that
	content: its tags stay glued to the text around them, and the special element's own tags break
	only for its content edges and the width, the way an inline element's or a component's do.
-->

<pre>a<svelte:element this="b"
		>{#if cond}text1   text2{/if}</svelte:element
	>c</pre>

<pre>a<svelte:element this="b"
		>{#each items as item}text1   {item}{/each}</svelte:element
	>c</pre>

<pre>a<svelte:component this={Comp}
		>{#key expr}text1   text2{/key}</svelte:component
	>c</pre>

<pre>a<slot>{#await promise}text1   text2{:then value}{value}{/await}</slot>c</pre>

<!-- the same content in an inline element and in a component -->
<pre>a<b
		>{#if cond}text1   text2{/if}</b
	>c</pre>

<pre>a<Comp
		>{#if cond}text1   text2{/if}</Comp
	>c</pre>
