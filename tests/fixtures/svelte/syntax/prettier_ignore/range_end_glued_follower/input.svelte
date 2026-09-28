<!-- prettier-ignore-start --><b >text1</b><!-- prettier-ignore-end -->text2

<!-- Inline content glued to a range's end marker stays glued: nothing separates them on a page -->
text1<!-- prettier-ignore-start --><b >text2</b><!-- prettier-ignore-end -->text3

<!-- prettier-ignore-start -->text1<!-- prettier-ignore-end -->text2

<!-- prettier-ignore-start --><b >text1</b><!-- prettier-ignore-end --><span>inline1</span>

<!-- prettier-ignore-start --><b >text1</b><!-- prettier-ignore-end --><Comp />

<!-- prettier-ignore-start --><b >text1</b><!-- prettier-ignore-end -->{expr}

<!-- prettier-ignore-start --><b >text1</b><!-- prettier-ignore-end -->{@html expr}

<!-- prettier-ignore-start --><b >text1</b><!-- prettier-ignore-end -->{#if cond}text2{/if}

<!-- prettier-ignore-start --><b >text1</b><!-- prettier-ignore-end -->{#if cond}text2{/if}text3

<!-- prettier-ignore-start -->text1<!-- prettier-ignore-end -->{#each items as item}{item}{/each}

<!-- prettier-ignore-start -->
<b >text1</b>
<!-- prettier-ignore-end --><svelte:element this="p">text2</svelte:element>

<!-- prettier-ignore-start --><b >text1</b><!-- prettier-ignore-end --><!-- c -->

<!-- prettier-ignore-start -->text1<!-- prettier-ignore-end --><!-- prettier-ignore-start -->
<b >text2</b>
<!-- prettier-ignore-end -->

<!-- A space after the end marker stays a space on the marker's line -->
<!-- prettier-ignore-start --><b >text1</b><!-- prettier-ignore-end --> text2

<!-- prettier-ignore-start --><b >text1</b><!-- prettier-ignore-end --> <span>inline1</span>

<!-- A range ending the document, glued to the text before it -->
text1<!-- prettier-ignore-start --><b >text2</b><!-- prettier-ignore-end -->

<!-- A global element or a snippet glued on both sides stays glued -->
<!-- prettier-ignore-start -->text1<!-- prettier-ignore-end --><svelte:window />text2

<!-- prettier-ignore-start --><!-- prettier-ignore-end --><svelte:head><meta /></svelte:head>text1

<!-- prettier-ignore-start -->text1<!-- prettier-ignore-end -->{#snippet fn()}text2{/snippet}text3

<!-- A block element after the end marker takes its own line, glued or not, as after any comment -->
<!-- prettier-ignore-start -->text1<!-- prettier-ignore-end -->
<div>block1</div>

<!-- prettier-ignore-start -->text1<!-- prettier-ignore-end -->
<pre>text2</pre>

<!-- A frozen global element with text after it: the boundary keeps its spelling -->
text1<!-- prettier-ignore-start --><svelte:body /><!-- prettier-ignore-end -->
text2

<!-- and spelled as a space -->
text1<!-- prettier-ignore-start --><svelte:document /><!-- prettier-ignore-end --> text2

<!-- The format-ignore spelling -->
text1<!-- format-ignore-start -->text2<!-- format-ignore-end -->text3
