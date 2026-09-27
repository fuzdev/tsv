<!--
	A frozen nested `<script>` / `<style>` glued to the text before it, with whitespace after it
	and an element, a component or an expression tag following, takes the line break after it that
	the same element takes unfrozen. It renders no box, so its neighbours meet across that
	whitespace and render one space either way; the directive glued in front of it stays glued.
-->

<!-- an inline element after -->
<div>
   text5<!-- prettier-ignore --><script>let  b  =  2;</script>   <b>inline1</b>   
</div>

<!-- a style, an inline element after -->
<div>
   text6<!-- prettier-ignore --><style>div  {  color:  blue;  }</style>   <b>inline2</b>   
</div>

<!-- a component after -->
<div>
   text7<!-- prettier-ignore --><script>let  c  =  3;</script>   <Comp />   
</div>

<!-- an expression tag after -->
<div>
   text8<!-- prettier-ignore --><script>let  d  =  4;</script>   {expr}   
</div>
