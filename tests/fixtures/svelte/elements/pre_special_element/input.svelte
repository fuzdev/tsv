<!-- Special elements (slot / svelte:*) inside <pre>: whitespace is significant, so they format inline (via the normal special-element builder) without injecting rendered whitespace -->
<pre><slot /></pre>

<pre><slot name="x">text</slot></pre>

<pre><svelte:element this="span">text</svelte:element></pre>

<pre><svelte:self /></pre>

<pre><svelte:component this={Comp} /></pre>

<pre><svelte:fragment>text</svelte:fragment></pre>
