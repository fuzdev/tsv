<!-- Short handler - stays completely inline -->
<button on:click={() => fn(aaaa)}>text</button>

<!-- 100 chars (at boundary) - attr fits inline so prettier hugs the content; tsv wraps the attr and lays content block-style -->
<button
	on:click={() => fn(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, lllll)}
>
	t
</button>

<!-- 101 chars - multiline attributes (exceeds boundary) -->
<button
	on:click={() => fn(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llllll)}
>
	t
</button>

<!-- Indented attribute: 100 chars - arrow body stays inline -->
<button
	on:click={() => fn(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llllll, mmm)}
>
	t
</button>

<!-- Indented attribute: 101 chars - arrow body wraps -->
<button
	on:click={() =>
		fn(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llllll, mmmm)}
>
	t
</button>

<!-- Call args: 100 chars - stays inline -->
<button
	on:click={() =>
		fn(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llllll, mmmm, nnnn, ooooo)}
>
	t
</button>

<!-- Call args: 101 chars - wraps to one-per-line -->
<button
	on:click={() =>
		fn(
			aaaa,
			bbbb,
			cccc,
			dddd,
			eeee,
			ffff,
			gggg,
			hhhh,
			iiii,
			jjjj,
			kkkk,
			llllll,
			mmmm,
			nnnn,
			oooooo
		)}
>
	t
</button>

<!-- With modifiers: 100 chars (at boundary) - attr fits inline so prettier hugs; tsv wraps the attr and lays content block-style -->
<button
	on:click|preventDefault={() => fn(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jj)}
>
	t
</button>

<!-- With modifiers: 101 chars - multiline -->
<button
	on:click|preventDefault={() => fn(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjj)}
>
	t
</button>

<!-- Block body - always expands -->
<button
	on:click={() => {
		aaaa;
		bbbb;
	}}
>
	t
</button>

<!-- Expression content: 100 chars - stays fully inline -->
<button on:click={() => fn(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjaa)}>{x}</button>

<!-- Expression content: 101 chars - SplitFinalBracket mode -->
<button on:click={() => fn(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjaaa)}>
	{x}
</button>

<!-- Expression content: 102 chars - Hug mode -->
<button on:click={() => fn(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjaaaa)}>
	{x}
</button>
