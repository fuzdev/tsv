<!--
	A special element inside `<pre>` keeps its content exactly as authored, the way a component or
	an inline element there does: its interior runs, tabs and edges are rendered text.
-->

<!-- `<svelte:element>`, with a static and a dynamic `this` -->
<pre>a<svelte:element this="b">text1   text2</svelte:element>c</pre>

<pre>a<svelte:element this={tag}> text1	text2 </svelte:element>c</pre>

<!-- `<svelte:component>` -->
<pre>a<svelte:component this={Comp}>text1   text2</svelte:component>c</pre>

<!-- `<svelte:self>`, inside a block where Svelte allows it -->
<pre>{#if cond}a<svelte:self>text1   text2</svelte:self>c{/if}</pre>

<!-- `<svelte:boundary>`, with a snippet among its content -->
<pre><svelte:boundary>text1  text2{#snippet failed()}text3  text4{/snippet}</svelte:boundary></pre>

<!-- `<svelte:fragment>`, as a component's direct child -->
<pre>a<Comp><svelte:fragment slot="name">text1   text2</svelte:fragment></Comp>c</pre>

<!-- `<slot>` fallback content -->
<pre>a<slot>text1   text2</slot>c</pre>

<!-- an empty body prints like any empty element -->
<pre>a<svelte:element this="b"></svelte:element>c</pre>

<pre>a<svelte:boundary></svelte:boundary>c</pre>
