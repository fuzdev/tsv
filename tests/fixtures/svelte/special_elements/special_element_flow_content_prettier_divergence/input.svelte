<!--
	Outside a whitespace-sensitive element a special element's content is flow content: a run of
	whitespace collapses, and content too wide for the line breaks onto its own indented line.
-->

<svelte:element this="b">text1 text2</svelte:element>

<svelte:element this="b">
	text1 text2 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
</svelte:element>

<svelte:element this={tag}>
	text1 text2 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
</svelte:element>

<svelte:component this={Comp}>
	text1 text2 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
</svelte:component>

{#if cond}
	<svelte:self>
		text1 text2 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
	</svelte:self>
{/if}

<svelte:boundary>
	text1 text2 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
</svelte:boundary>

<Comp>
	<svelte:fragment slot="name">
		text1 text2 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
	</svelte:fragment>
</Comp>

<slot>
	text1 text2 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
</slot>
