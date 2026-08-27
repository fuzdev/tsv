<!--
	Svelte hands acorn a MANUFACTURED source at four of the parses below, and acorn's
	`onComment` dedents a multiline block comment by the indentation of the line THAT source
	opens with, which is not the document's. Every case here opens its comment on the line the
	manufacture covers, so the indentation acorn measured is the manufacture's own and the tab
	the author wrote survives into the wire `value`. The four are one rule read at one place
	each: a block binding's `: T` reads `<spaces>_ as T`, a destructuring binding reads
	`<spaces>(pattern = 1)`, and a `{#snippet}` head reads a prelude that blanks only the
	NON-whitespace.
	`{@const b = ...}` and `{expr ...}` are the null controls: acorn reads those out of the raw
	template, so the line it measured IS the document's and the tab comes off.
	The `prettier-ignore` is what keeps the triggers alive. Both formatters reflow every one of
	these heads onto a line the manufacture no longer reaches, so an unfrozen spelling of this
	file turns each case into the control rather than failing here. The `lang="ts"` is for the
	`{#each xs as x: T}` annotation alone.
-->
<script lang="ts">
	let expr = 1;
	let xs = [1];
	let p = Promise.resolve(1);
</script>

<!-- prettier-ignore -->
<div>
	{#each xs as x: /*
	 c1 */ number}{x}{/each}
	{#if expr}
		{@const { a = /*
		 c2 */ 1 } = { a: 1 }}
		{@const b = /*
		 c3 */ 1}
		{a}{b}
	{/if}
	{#await p then { c = /*
	 c4 */ 1 }}{c}{/await}
	{#snippet s(d = /*
	 c5 */ 1)}{d}{/snippet}
	{expr /*
	 c6 */}
</div>
