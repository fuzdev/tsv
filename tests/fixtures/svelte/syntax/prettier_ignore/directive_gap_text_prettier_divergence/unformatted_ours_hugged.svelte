<!--
	A frozen TEXT node carries the gap inside its own slice — the parser folds the run
	before it into the node — so the break in front of it is already in the frozen bytes
	and the printer must not print a second one. Its trailing run goes wherever the boundary
	after it is the printer's — the fragment's own close, a follower that owns its line, a
	whitespace-collapsing container — and stays where the follower would print nothing
	beside a slice that already ends in a break.
-->

{#if cond}<!-- prettier-ignore -->

text1{/if}

{#if cond}text0

<!-- prettier-ignore -->text1{/if}

<div>text0

<!-- prettier-ignore -->

text1</div>

<div><!-- prettier-ignore -->

text1
<span>text2</span></div>

{#if cond}<!-- prettier-ignore -->

text1

{@const y = 1}{/if}

{#if cond}<!-- prettier-ignore -->

text1
{#snippet s()}b{/snippet}{/if}

<select><!-- prettier-ignore -->

text1
<option>a</option></select>
