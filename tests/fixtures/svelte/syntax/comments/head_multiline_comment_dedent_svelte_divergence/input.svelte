<!--
	Each case opens a multiline block comment on the line where one of Svelte's MANUFACTURED
	sources ends. acorn's `onComment` dedents it by the indentation of its line in the string
	acorn was handed, and these readers hand it a rewritten one: a block binding's `: T` reads
	`<spaces>_ as T`, a destructuring binding reads `<spaces>(pattern = 1)`, and a `{#snippet}`
	head reads a prelude that blanks only the NON-whitespace. Svelte therefore measures a run
	the document does not hold, and the tab the author wrote survives into its `value`. tsv
	measures the line the document holds, so the tab comes off.
	`{@const b = ...}` and `{expr ...}` are the controls: acorn reads those out of the raw
	template, so both parsers measure the document's own line and agree.
	The `prettier-ignore` is what keeps the triggers alive. Both formatters reflow every one of
	these heads onto a line the manufacture no longer reaches, so an unfrozen spelling of this
	file turns each case into a control. The `lang="ts"` is for the `{#each xs as x: T}`
	annotation alone.
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
