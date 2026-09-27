<!--
	A frozen node whose bytes span lines breaks the block body holding it, which lays out
	block-style: the body moves to its own indented lines, and the frozen bytes print as written.
-->

<!-- a frozen element first, in an if body -->
{#if cond}<!-- prettier-ignore --><b   data-attr="value"
>inline1</b>text1{/if}

<!-- a frozen element between two texts, in an each body -->
{#each items as item}text2<!-- prettier-ignore --><b   data-attr="value"
>inline2</b>{item}{/each}

<!-- a frozen expression tag last, in an else branch -->
{#if cond}text3{:else}text4<!-- prettier-ignore -->{a  +
  b}{/if}

<!-- a frozen element in a snippet body -->
{#snippet fn()}text5<!-- prettier-ignore --><b   data-attr="value"
>inline3</b>text6{/snippet}

<!-- a frozen element in an await phase -->
{#await promise}text7<!-- prettier-ignore --><b   data-attr="value"
>inline4</b>text8{:then value}{value}{/await}

<!-- a frozen element in a key body -->
{#key cond}text9<!-- prettier-ignore --><b   data-attr="value"
>inline5</b>text10{/key}

<!-- a frozen element on one line stays inline -->
{#if cond}text11<!-- prettier-ignore --><b   data-attr="value">inline6</b>text12{/if}

<!-- the same element unfrozen collapses -->
{#if cond}text13<b data-attr="value">inline7</b>text14{/if}
