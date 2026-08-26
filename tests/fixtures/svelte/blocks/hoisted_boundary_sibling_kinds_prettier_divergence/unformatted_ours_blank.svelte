<!--
	The hoisted-edge trim reads the EDGE, not the neighbour's kind: `clean_nodes` lifts a
	`{@debug}` out of the fragment before it trims, so whatever sibling stands beside it is the
	fragment's first/last node and the run between them is render-free. A sibling whose own
	newline flows — an inline element, a component, a tag — therefore reaches the glued form, as
	a text sibling already does. A sibling that owns its line keeps the run as its separator,
	and so does one that is itself hoisted — the fragment then has no content end at all.
-->

<!-- trailing edge, every sibling kind whose newline flows -->
<div><span>inline1</span>

	{@debug expr}</div>
<div><Comp />

	{@debug expr}</div>
<div>{expr}

	{@debug expr}</div>
<div>{@html expr}

	{@debug expr}</div>
<div>{@render fn()}

	{@debug expr}</div>

<!-- leading edge -->
<div>{@debug expr}

	<span>inline1</span></div>
<div>{@debug expr}

	{expr}</div>

<!-- a block BODY: the run is deleted, so a blank at this edge cannot force the body open —
     the fragment-level blank gate must read the same edge (blocks/body_blank_break) -->
{#if cond}<span>inline1</span>

{@debug expr}{/if}
{#each items as item}{expr}

{@debug expr}{/each}

<!-- the text sibling this extends -->
<div>text1

	{@debug expr}</div>

<!-- interior control: with content on both sides the runs merge into one rendered space -->
<div><span>inline1</span> {@debug expr} <span>inline2</span></div>

<!-- controls: a sibling that owns its own line keeps the run — a comment, a <br />, a
     control-flow block, a block element -->
<div>
	<!-- c1 -->
	{@debug expr}
</div>
<div>
	<br />
	{@debug expr}
</div>
<div>
	{#if cond}text1{/if}
	{@debug expr}
</div>
<div>
	<div>block1</div>
	{@debug expr}
</div>

<!-- controls: a hoisted <title>, in one <svelte:head> because a component may only have one.
     The nested bodies go FIRST so `<title>text2</title>` keeps the trailing edge its own case
     needs. As the HOISTED end it keeps its line among element siblings. As the CONTENT end it
     keeps the run too, because both ends are then hoisted and the fragment has no content end
     at all — a <title> is not block-classified, so it FLOWS, and the neighbour-kind test alone
     welds it. The <b> twin varies only that one node's kind and does weld -->
<svelte:head>
	{#if cond}<title>text3</title> {@debug expr}{/if}
	{#if cond}<b>text3</b>{@debug expr}{/if}
	<b>text1</b>
	<title>text2</title>
</svelte:head>

<!-- control: the same no-content-end class spelled with a hoisted kind that does not flow -->
{@debug expr}

{@debug expr2}
