<!--
	A declaration tag renders nothing and takes its OWN LINE: `{@const}`, and the
	`{const …}` / `{let …}` pair. `clean_nodes` HOISTS it out of its fragment before the
	whitespace rules run, so the break beside it is render-free and every authoring
	converges here. Two shapes are excluded: a tag GLUED on both sides (breaking there
	WOULD inject a rendered space) and `{@debug}`, whose edge run trims instead.
-->
{#if cond}{@const x = 1} text1{/if}

{#if cond}text2 {@const x = 1}{/if}

<!-- Interior: `a {@const} b` renders `a b` whichever line the tag is on -->
{#if cond}a {@const x = 1} b{/if}

<!-- GLUED on both sides: `a{@const}b` renders `ab`, so the tag keeps the author's line -->
{#if cond}a{@const x = 1}b{/if}

<!-- A lone tag still takes its line: the block goes multiline -->
{#if cond}
	{@const x = 1}
{/if}

<!-- Consecutive tags each take a line: a hoisted neighbour is not content -->
{#if cond}{@const x = 1} {@const y = 2} text3{/if}

<!-- `{const}` / `{let}` are legal in every fragment, so the rule reaches element content -->
<div>text4 {const x = 1} text5</div>

<!-- `{@debug}` is excluded: its edge run TRIMS instead -->
{#if cond}text6 {@debug cond}{/if}

<!-- A component hosts a fragment too, and a `{#snippet}` sibling hoists for the compiler alike -->
<Comp>{@const a = 1} {#snippet f()}x{/snippet}</Comp>
