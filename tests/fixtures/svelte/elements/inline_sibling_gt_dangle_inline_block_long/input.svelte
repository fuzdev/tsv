<!-- A wide inline element glued to a SHORT block: the element's closing > dangles onto the block-head line and the block stays inline -->
<div>
	<span data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">text</span
	>{#if cond}, text1 text2{/if}
</div>

<!-- {#each} dangles the same way -->
<div>
	<span data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">text</span
	>{#each items as item}{item}{/each}
</div>

<!-- an element body stays inline too -->
<div>
	<span data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">text</span
	>{#if cond}<span>inline1</span>{/if}
</div>

<!-- boundary: the hugged line at exactly 100 stays hugged -->
<div>
	<span data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">text</span>{#if cond}text{/if}
</div>

<!-- boundary: at 101 the > dangles and the block stays inline -->
<div>
	<span data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">text</span
	>{#if cond}text{/if}
</div>
