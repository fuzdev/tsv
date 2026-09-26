<!-- Special elements (slot / svelte:*) inside <pre>: their content is <pre> content, laid out by the whitespace-sensitive builders, so nothing adds rendered whitespace; an empty or self-closing one keeps its own tag form -->
<pre><slot /></pre>

<pre><slot name="x">text</slot></pre>

<pre><svelte:element this="span">text</svelte:element></pre>

{#if cond}<pre><svelte:self /></pre>{/if}

<pre><svelte:component this={Comp} /></pre>

<pre><svelte:fragment>text</svelte:fragment></pre>
