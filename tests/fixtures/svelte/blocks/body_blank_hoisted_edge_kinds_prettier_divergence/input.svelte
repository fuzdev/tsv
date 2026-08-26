<!--
	A blank line at a hoisted body EDGE carries a Tier-2 signal unless the printer actually
	DELETES the run it sits in. That trim is narrow — the hoisted end must be a `{@debug}` and
	the content end a sibling whose own newline flows (blocks/hoisted_boundary_sibling_kinds) —
	so at every kind it excludes, the run survives and the blank with it: the body opens, like
	any other interior blank (blocks/body_blank_break). Prettier keeps the blank in every case
	here, welding the body to its head and close.
-->

<!-- the content end owns its line: a comment, at either edge -->
{#if cond}
	<!-- c -->

	{@debug expr}
{/if}
{#if cond}
	{@debug expr}

	<!-- c -->
{/if}

<!-- the content end owns its line: a `<br />`, at either edge -->
{#if cond}
	<br />

	{@debug expr}
{/if}
{#if cond}
	{@debug expr}

	<br />
{/if}

<!-- not keyed to `{#if}`: the same comment edge in an `{#each}` body -->
{#each items as item}
	<!-- c -->

	{@debug expr}
{/each}

<!-- the HOISTED end is not a `{@debug}`, so the SEPARATOR run survives and carries the blank —
	but beside a TEXT the very same `<title>` trims it away, because that run is the content
	text's own edge and a different emitter deletes it. The split is the EMITTER, not the hoisted
	kind. Third: inside a `<div>` head context stops, so the `<title>` is an ordinary element,
	nothing is hoisted, and no trim is in play at all -->
<svelte:head>
	{#if cond}
		<b>text1</b>

		<title>text2</title>
	{/if}
	{#if cond}text1<title>text2</title>{/if}
	<div>
		{#if cond}
			text1

			<title>text2</title>
		{/if}
	</div>
</svelte:head>

<!-- controls: the run IS deleted, so the blank carries nothing and the body stays hugged —
	every content-end kind whose newline flows -->
{#if cond}<span>inline1</span>{@debug expr}{/if}
{#if cond}text1{@debug expr}{/if}
{#if cond}{expr}{@debug expr}{/if}
{#if cond}<Comp />{@debug expr}{/if}

<!-- control: neither end is content, so nothing is weldable and nothing carries a signal -->
{#if cond}{@debug expr}<span>inline1</span>{@debug expr2}{/if}

<!-- control: the same excluded kinds with a SPACE rather than a blank — no Tier-2 signal, so
	nothing forces the body and the hug stays. This is the form eating the blank produces, which is
	why it has to be correct HERE and wrong there -->
{#if cond}<!-- c --> {@debug expr}{/if}
{#if cond}<br /> {@debug expr}{/if}
