<!--
	A frozen nested `<script>` / `<style>` glued to the content before it, with whitespace after it,
	takes the line break after it that the same element takes unfrozen. It renders no box, so its
	neighbours meet across that whitespace and render one space either way; the directive glued in
	front of it stays glued.
-->

<!-- a script, text after -->
<div>text1<!-- prettier-ignore --><script>let  a  =  1;</script> text2</div>

<!-- a style -->
<div>text3<!-- prettier-ignore --><style>div  {  color:  red;  }</style> text4</div>

<!-- a block body -->
{#if cond}text9<!-- prettier-ignore --><script>let  e  =  5;</script> text10{/if}
