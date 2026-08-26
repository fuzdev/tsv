<!--
	An authored blank line inside a block BODY is a Tier-2 signal, so it forces the body open:
	the construct expands and the blank survives, at every block kind and in both spellings the
	parser gives a blank (a whitespace-only node between two siblings, or a content text's edge).
	Three shapes are NOT the signal, one control each: a blank INTERIOR to one text (the fill
	collapses it), a blank on the body's own BOUNDARY (render-free air, trimmed at either end),
	and one whose run a hoisted `{@debug}` TRIMS away at the body's edge — in either spelling,
	since the trim is what deletes it. The same hoisted node with content on BOTH sides merges
	its runs rather than deleting them, so the blank there is interior content and survives.
	`<pre>` is whitespace-significant, so the gate never reaches it at all.
-->

<!-- every block kind: the blank sits in a whitespace-only node between two siblings -->
{#if cond}<span>inline1</span>

<span>inline2</span>{/if}
{#each items as item}<span>inline1</span>

<span>inline2</span>{/each}
{#key key}<span>inline1</span>

<span>inline2</span>{/key}
{#await promise then value}<span>inline1</span>

<span>inline2</span>{/await}
{#snippet fn()}<span>inline1</span>

<span>inline2</span>{/snippet}
{#if cond}text1{:else}<span>inline1</span>

<span>inline2</span>{/if}

<!-- every other section and branch marker: the body it introduces breaks the same way -->
{#if cond}text1{:else if other}<span>inline1</span>

<span>inline2</span>{/if}
{#await promise}text1{:then value}text2{:catch error}<span>inline1</span>

<span>inline2</span>{/await}
{#each items as item}text1{:else}<span>inline1</span>

<span>inline2</span>{/each}
<Comp>{#snippet row()}<span>inline1</span>

<span>inline2</span>{/snippet}</Comp>

<!-- the other spelling: the blank is folded into a content text's edge whitespace -->
{#if cond}text1 text2

<span>inline1</span>{/if}
{#if cond}<span>inline1</span>

text1 text2{/if}

<!-- a hoisted {@debug} with content on BOTH sides merges its runs instead of deleting them,
	so a blank beside it is interior content and breaks the body like any other -->
{#if cond}a

{@debug cond}

b{/if}

<!-- control: a blank INTERIOR to one text is the fill's, so the body stays hugged -->
{#if cond}<code>a</code> text1 text2

text3 text4{/if}

<!-- control: no blank, so the body stays hugged -->
{#if cond}<span>inline1</span> <span>inline2</span>{/if}

<!-- control: a hoisted {@debug} at the body's EDGE trims its run, blank and all — in the text
	spelling and in the whitespace-only-node spelling alike -->
{#if cond}text1

{@debug cond}{/if}
{#if cond}<span>inline1</span>

{@debug cond}{/if}
{#if cond}{@debug cond}

<span>inline1</span>{/if}
{#each items as item}<span>inline1</span>

{@debug cond}{/each}

<!-- control: <pre> is whitespace-significant, so the body is never reshaped -->
<pre>{#if cond}<span>inline1</span>

<span>inline2</span>{/if}</pre>
