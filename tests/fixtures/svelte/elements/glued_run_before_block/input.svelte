<!-- A glued run of inline elements directly before an inline block: the whole run prints, the block stays inline -->
<div>
	<span>inline1</span><span>inline2</span>{#if cond}text{/if}
</div>

<!-- three-element run before {#each} -->
<div>
	<span>inline1</span><span>inline2</span><span>inline3</span>{#each items as item}{item}{/each}
</div>

<!-- a glued comment prefix on the element prints too -->
<div>
	<!-- comment --><span>inline1</span>{#key expr}text{/key}
</div>

<!-- {#await} -->
<div>
	<span>inline1</span><span>inline2</span>{#await promise}text{/await}
</div>

<!-- {#snippet} glued to content on both sides -->
<div>
	<span>inline1</span><span>inline2</span>{#snippet fn()}text1{/snippet}text2
</div>
