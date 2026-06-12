<p>
	<slot
		data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
		data-attr2="bbbbbbbbbbbbbbbbbbbbb">{expr}</slot
	>{#if cond}text{/if}
</p>

<p>
	<svelte:element this={a ? 'a' : 'span'} data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
		>{expr}</svelte:element
	>{#if cond}text{/if}
</p>

<p>
	<slot data-attr="aaa">{expr}</slot>{#if cond}text{/if}
</p>
