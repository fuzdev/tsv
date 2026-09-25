<!--
	An inline element glued to a following block that renders multiline, when the element is too
	wide for its line. Its content drops to its own indented line and so does its closing tag, and
	the element's closing > dangles onto the block-head line as it does for an element that fits.
	The same holds for the four rendering block heads, the tail of a glued run, and an element
	behind a glued comment. An element multiline only by authored line breaks dangles its > the
	same way; one multiline by structure (a block child) keeps it. Prettier prints each block-style
	element the same, but hugs its > to the block head — see output_prettier.svelte.
-->

<!-- 100: the shedder's line up to its dangled > fits, so it stays flat -->
<b class="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc">{y}</b
>{#if c}
	x
{/if}

<!-- 101: the shedder's content drops to its own line and so does its closing tag -->
<b class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	>{y}</b
>{#if c}
	x
{/if}

<!-- an inline block, 100: the element's line up to its dangled > fits, so it stays flat -->
<b class="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc">{y}</b
>{#if c}t{/if}

<!-- an inline block, 101: the element goes block-style and its > hugs the block, which fits -->
<b class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	>{y}</b
>{#if c}t{/if}

<!-- the same element multiline by authored line breaks, its > hugging the inline block -->
<b>
	{y}
</b>{#if c}t{/if}

<!-- {#each} -->
<b class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	>{y}</b
>{#each items as item}
	x
{/each}

<!-- {#await} -->
<b class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	>{y}</b
>{#await p}
	x
{/await}

<!-- {#key} -->
<b class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	>{y}</b
>{#key k}
	x
{/key}

<!-- a block with an {:else} branch -->
<b class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	>{y}</b
>{#if c}
	x
{:else}
	y
{/if}

<!-- text in the element -->
<b class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	>x</b
>{#if c}
	x
{/if}

<!-- a component -->
<Comp class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	>{y}</Comp
>{#if c}
	x
{/if}

<!-- the tail of a glued run -->
<u>a</u><b class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	>{y}</b
>{#if c}
	x
{/if}

<!-- behind a glued comment -->
<!-- c --><b class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	>{y}</b
>{#if c}
	x
{/if}

<!-- a shedder multiline only by authored line breaks dangles its > like its width-broken twin -->
<b>
	{y}
</b>{#if c}
	x
{/if}

<!-- the same as the tail of a glued run -->
<u>a</u><b>
	{y}
</b>{#if c}
	x
{/if}

<!-- the same behind a glued comment -->
<!-- c --><b>
	{y}
</b>{#if c}
	x
{/if}

<!-- control: a shedder multiline by structure (a block child) keeps its > intact -->
<a href="/"
	>{#if x}a{/if}</a
>{#each items as item}
	<span>{item}</span>
{/each}

<!-- in a block parent -->
<div>
	<b class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
		>{y}</b
	>{#if c}
		x
	{/if}
</div>

<!-- in an inline parent -->
<span
	><b class="ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
		>{y}</b
	>{#if c}
		x
	{/if}</span
>
