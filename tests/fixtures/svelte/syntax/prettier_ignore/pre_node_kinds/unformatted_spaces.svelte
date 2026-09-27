<!--
	A `prettier-ignore` inside `<pre>` freezes the next node, as it does outside one: the node keeps
	its source text, and the unfrozen twin after it formats.
-->

<!-- an element -->
<pre>a<!-- prettier-ignore --><b   data-a = "x" >text1</b><b  data-a = "x" >text1</b>c</pre>

<!-- void elements -->
<pre>a<!-- prettier-ignore --><br   ><br  />c</pre>

<pre>a<!-- prettier-ignore --><img   src = "x" ><img  src = "x"  />c</pre>

<!-- a component, with content and self-closing -->
<pre>a<!-- prettier-ignore --><Comp   a = "1" >text2</Comp><Comp  a = "1" >text2</Comp>c</pre>

<pre>a<!-- prettier-ignore --><Comp   a = "1"  /><Comp  a = "1"  />c</pre>

<!-- an expression tag and an `{@html}` tag -->
<pre>a<!-- prettier-ignore -->{  expr  +  b  }{  expr  +  b  }c</pre>

<pre>a<!-- prettier-ignore -->{@html   expr  }{@html   expr  }c</pre>

<!-- control-flow blocks -->
<pre>a<!-- prettier-ignore -->{#if   cond}text3{/if}{#if   cond}text3{/if}c</pre>

<pre>a<!-- prettier-ignore -->{#each arr   as i}text4{/each}{#each  arr  as  i}text4{/each}c</pre>

<!-- a special element -->
<pre>a<!-- prettier-ignore --><svelte:element   this="i" >text5<br   ></svelte:element>c</pre>

<pre>a<svelte:element  this="i" >text5<br  /></svelte:element>c</pre>

<!-- a nested `<textarea>` -->
<pre>a<!-- prettier-ignore --><textarea   rows = "2" >text6</textarea>c</pre>

<pre>a<textarea  rows = "2" >text6</textarea>c</pre>

<!-- the directive with a space, a line break or a blank line before the node -->
<pre>a<!-- prettier-ignore --> <b   data-a = "x" >text7</b><b  data-a = "x" >text7</b>c</pre>

<pre>a<!-- prettier-ignore -->
<b   data-a = "x" >text8</b><b  data-a = "x" >text8</b>c</pre>

<pre>a<!-- prettier-ignore -->

<b   data-a = "x" >text9</b><b  data-a = "x" >text9</b>c</pre>

<!-- a comment before the directive -->
<pre>a<!-- c --><!-- prettier-ignore --><b   data-a = "x" >text10</b><b  data-a = "x" >text10</b>c</pre>

<!-- a run of spaces and a tab between the directive and the node -->
<pre>a<!-- prettier-ignore -->   	  <b   data-a = "x" >text11</b><b  data-a = "x" >text11</b>c</pre>

<!-- the directive after the `<pre>`'s own leading line break -->
<pre>
<!-- prettier-ignore --><b   data-a = "x" >text12</b><b  data-a = "x" >text12</b></pre>

<!-- a frozen nested `<pre>`, whose own leading line break HTML drops too -->
<pre>a<!-- prettier-ignore --><pre   class = "x" >
text13</pre><pre  class = "x" >
text13</pre>c</pre>

<!-- a frozen node whose content opens and closes on a line break, and one after it -->
<pre>a<!-- prettier-ignore --><b   data-a = "x" >
text14
</b>
<b  data-a = "x" >
text14
</b>c</pre>

<!-- multibyte text before and inside the frozen node -->
<pre>é😀<!-- prettier-ignore --><b   data-a = "ü" >😀text15</b><b  data-a = "ü" >😀text15</b>c</pre>

<!-- a comment inside a frozen expression tag, and a directive inside a frozen element -->
<pre>a<!-- prettier-ignore -->{/* c */  a  +  b}{/* d */  a  +  b }c</pre>

<pre>a<!-- prettier-ignore --><b   a = "1" ><!-- prettier-ignore --><i   a = "1" >x</i></b>c</pre>

<!-- a multi-line frozen node inside an indented `<pre>` keeps its lines at their columns -->
<div>
	<pre>a<!-- prettier-ignore --><b   data-a = "x" >text16
  text17</b>c</pre>
</div>

<!--
	Where the directive freezes nothing new: a second directive is the node the first one freezes,
	so the element after it formats, inside `<pre>` and outside it; range markers inside an element
	are ordinary comments; and a directive that ends its container has no next node.
-->
<pre>a<!-- prettier-ignore --><!-- prettier-ignore --><b  data-a = "x" >text18</b>c</pre>

<p>a<!-- prettier-ignore --><!-- prettier-ignore --><b  data-a = "x" >text18</b>c</p>

<pre>a<!-- prettier-ignore-start --><b  data-a = "x" >text19</b><!-- prettier-ignore-end -->c</pre>

<pre>a<!-- format-ignore-start --><b  data-a = "x" >text19</b><!-- format-ignore-end -->c</pre>

<pre>a<b>text20<!-- prettier-ignore --></b>c</pre>

<pre>{#if cond}text20<!-- prettier-ignore -->{/if}</pre>

<pre>a<!-- prettier-ignore --></pre>
