text1 text2 <svelte:head><title>x</title></svelte:head>

<!--
	The block-after-spaced-text rule across every block KIND and every fragment FAMILY the main
	fixture does not vary: a `<pre>`, a void `<hr />`, a whitespace-collapsing `<table>`, a list, a
	heading, the block-classified `svelte:*` specials (`<svelte:head>` above at the root,
	`<svelte:window />` below), a block whose attributes wrap, a one-punctuation text, and the
	`{#each}` / `{#snippet}` / `{:else}` / `{#await}` bodies and the `<slot>` / `<svelte:boundary>`
	containers. Prettier hugs each to a first-child text (`prettier_variant_space.svelte`); tsv
	normalizes that spelling, and the tab spelling (`unformatted_ours_tab.svelte`), to this file in
	one pass.
-->

<!-- every block kind: pre, hr, table (block-style, a whitespace-collapsing container), ul, h1 -->
<div>
	text1 text2 <pre>x</pre>
</div>
<div>
	text1 text2 <hr />
</div>
<div>
	text1 text2 <table><tr><td>x</td></tr></table>
</div>
<div>
	text1 text2 <ul><li>x</li></ul>
</div>
<div>
	text1 text2 <h1>x</h1>
</div>

<!-- a block whose attributes wrap -->
<div>
	text1 text2 <div
		class="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
		data-attr="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
	>
		e
	</div>
</div>

<!-- a one-punctuation text -->
<div>
	( <div>e</div>
</div>

<!-- the other root-only special, after a comment -->
text1 text2 <svelte:window onkeydown={fn} />

<!-- every fragment family: each, snippet, else branch, await then, slot, svelte:boundary -->
{#each items as item}
	text1 text2 <div>{item}</div>
{/each}
{#snippet fn()}
	text1 text2 <div>e</div>
{/snippet}
{#if a}
	x
{:else}
	text1 text2 <div>e</div>
{/if}
{#await p then v}
	text1 text2 <div>{v}</div>
{/await}
<slot>
	text1 text2 <div>e</div>
</slot>
<svelte:boundary>
	text1 text2 <div>e</div>
</svelte:boundary>
