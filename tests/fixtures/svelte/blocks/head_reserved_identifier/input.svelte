<!--
	A {#snippet} name, an {#each} index, and a plain-identifier block binding all go
	through Svelte's `read_identifier`, which rejects a reserved word. These near-misses
	are NOT on that list and stay valid; the input_invalid_* files carry the rejected
	spellings.
-->
{#snippet of()}
	<p>text1</p>
{/snippet}

{#snippet async()}
	<p>text2</p>
{/snippet}

{@render of()}
{@render async()}

{#each items as item, get}
	<p>text3 {get}</p>
{/each}

{#each items as set}
	<p>text4 {set}</p>
{/each}

{#await promise}
	<p>text5</p>
{:then from}
	<p>text6 {from}</p>
{:catch undefined}
	<p>text7 {undefined}</p>
{/await}
