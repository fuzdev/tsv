<!--
	A `prettier-ignore` inside a special element inside `<pre>` freezes the next node, as it does
	inside an element there: the special element's content is the `<pre>`'s content too.
-->

<!-- `<svelte:element>`, with a static and a dynamic `this` -->
<pre><svelte:element this="b">
<!-- prettier-ignore --><i   a = "1" >x</i><i a="1">x</i>
</svelte:element></pre>

<pre><svelte:element this={tag}>
<!-- prettier-ignore -->{  expr  }{expr}
</svelte:element></pre>

<!-- `<svelte:component>` -->
<pre><svelte:component this={Comp}>
<!-- prettier-ignore --><i   a = "1" >x</i><i a="1">x</i>
</svelte:component></pre>

<!-- `<svelte:self>`, inside a block where Svelte allows it -->
<pre>{#if cond}<svelte:self>
<!-- prettier-ignore --><i   a = "1" >x</i><i a="1">x</i>
</svelte:self>{/if}</pre>

<!-- `<svelte:boundary>`, and a snippet inside it -->
<pre><svelte:boundary>
<!-- prettier-ignore --><i   a = "1" >x</i><i a="1">x</i>
</svelte:boundary></pre>

<pre><svelte:boundary>
{#snippet failed()}<!-- prettier-ignore -->{  e  }{e}{/snippet}
</svelte:boundary></pre>

<!-- `<svelte:fragment>`, as a component's direct child -->
<pre><Comp>
<svelte:fragment slot="s"><!-- prettier-ignore --><i  a>x</i><i a>x</i></svelte:fragment>
</Comp></pre>

<!-- `<slot>` fallback content -->
<pre><slot>
<!-- prettier-ignore --><i   a = "1" >x</i><i a="1">x</i>
</slot></pre>

<!-- a special element frozen inside another -->
<pre><svelte:element this="b">
<!-- prettier-ignore --><svelte:boundary   >x<br   ></svelte:boundary>
<svelte:boundary>x<br /></svelte:boundary>
</svelte:element></pre>
