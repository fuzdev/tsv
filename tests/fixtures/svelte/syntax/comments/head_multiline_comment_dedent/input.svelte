<!--
	Each case opens a multiline block comment on the first line of a block head — beside a
	binding's `: T`, inside a destructuring binding, in a `{#snippet}` parameter list. acorn's
	`onComment` dedents such a comment by the indentation of the line it opens on, and that
	line is the document's: the tab the author wrote comes off the continuation line of
	every `value`, in both parsers. `{@const b = ...}` and `{expr ...}` are the plain cases
	beside them.
	The `prettier-ignore` is what keeps the comments on those lines — both formatters reflow
	every one of these heads. The `lang="ts"` is for the `{#each xs as x: T}` annotation alone.
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
