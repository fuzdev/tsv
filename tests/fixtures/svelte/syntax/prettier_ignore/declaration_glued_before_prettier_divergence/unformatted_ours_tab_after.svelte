<!--
	A frozen declaration tag glued to the content before it, with whitespace after it, takes the
	line break after it that the same tag takes unfrozen. The tag renders nothing and the compiler
	hoists it out of the fragment, so its neighbours meet across that whitespace and render one
	space either way; the directive glued in front of it stays glued.
-->

<!-- a let tag, text after -->
text1<!-- prettier-ignore -->{let   a = 1}	text2

<!-- a const tag -->
text3<!-- prettier-ignore -->{const   b = 2}	text4

<!-- a snippet -->
text5<!-- prettier-ignore -->{#snippet   fn()}text6{/snippet}	text7

<!-- an @const tag inside a component -->
<Comp>
	text8<!-- prettier-ignore -->{@const   c = 3}	text9
</Comp>

<!-- an @const tag inside a block body -->
{#if cond}
	text10<!-- prettier-ignore -->{@const   d = 4}	text11
{/if}

<!-- control: an element after -->
text12<!-- prettier-ignore -->{let   e = 5}	<span>inline1</span>
