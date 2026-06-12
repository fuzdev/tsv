<script>
	let a;
	let b;
</script>

<!-- Simple if - text after {/if} -->
<div>
	{#if a}
		<span>text1</span>
	{/if} text2
</div>

<!-- If-else - text in branch and after {/if} -->
<div>
	{#if a}
		<span>text1</span>
	{:else}
		<span>text2</span> text3
	{/if} text4
</div>

<!-- If-else-if - text after {/if} -->
<div>
	{#if a}
		<span>text1</span>
	{:else if b}
		<span>text2</span>
	{/if} text3
</div>

<!-- With components - text after {/if} -->
<div>
	{#if a}
		<Comp1 />
	{:else}
		<Comp2 /> text1
	{/if} text2
</div>

<!-- Entity after {/if} (already works) -->
<div>
	{#if a}
		<span>text1</span>
	{/if}&nbsp;text2
</div>

<!-- Multiple text nodes after {/if} -->
<div>
	{#if a}
		<span>text1</span>
	{/if} text2 text3
</div>
