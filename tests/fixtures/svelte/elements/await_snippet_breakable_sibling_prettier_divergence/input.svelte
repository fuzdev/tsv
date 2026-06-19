<!-- A short {#await}/{#snippet} after a breakable sibling (an inline element, an
expression tag, {@html}, or {@render}) inside a block element drops the children to their
own line — the block stays hugged to its sibling (no `>` to dangle when short), so only
the parent goes multiline. Prettier keeps an inline-authored construct inline. -->

<!-- inline-element sibling + await -->
<div>
	<span>s</span>{#await p}x{/await}
</div>

<!-- expression-tag sibling + await -->
<div>
	{e}{#await p}x{/await}
</div>

<!-- {@html} sibling + await -->
<div>
	{@html s}{#await p}x{/await}
</div>

<!-- {@render} sibling + await -->
<div>
	{@render f()}{#await p}x{/await}
</div>

<!-- inline-element sibling + snippet -->
<div>
	<span>s</span>{#snippet f()}x{/snippet}
</div>
