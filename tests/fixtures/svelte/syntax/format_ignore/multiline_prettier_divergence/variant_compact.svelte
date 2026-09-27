<!--
	A format-ignore'd node whose bytes span lines breaks the fragment holding it, as a
	prettier-ignore'd one does: a block element, an inline element and a block body alike.
-->

<!-- in a block element -->
<p>text1<!-- format-ignore --><b data-attr="value">inline1</b>text2</p>

<!-- in an inline element -->
<span>text3<!-- format-ignore --><b data-attr="value">inline2</b>text4</span>

<!-- in a block body -->
{#if cond}text5<!-- format-ignore --><b data-attr="value">inline3</b>text6{/if}
