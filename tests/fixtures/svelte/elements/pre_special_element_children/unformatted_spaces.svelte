<!--
	Inside `<pre>`, every child of a special element is laid out as `<pre>` content: the runs around
	an inline element, a block element, an expression, a comment, a raw HTML tag and a render tag
	are all rendered text, and a newline at the content's edges stays where it is.
-->

<pre>a<svelte:element  this="b" >text1  <i>text2  text3</i>  text4</svelte:element  >c</pre>

<pre>a<svelte:element  this="b" >text1  <div>block1</div>  text2</svelte:element  >c</pre>

<pre>a<svelte:element  this="b" >text1  {expr}  text2</svelte:element  >c</pre>

<pre>a<svelte:element  this="b" >text1  <!-- c -->  text2</svelte:element  >c</pre>

<pre>a<svelte:element  this="b" >text1  {@html expr}  text2</svelte:element  >c</pre>

<pre>a<svelte:element  this="b" >text1  {@render fn()}  text2</svelte:element  >c</pre>

<pre>a<svelte:element  this="b" >
text
</svelte:element  >c</pre>

<!-- a comment in the `this` binding -->
<pre>a<svelte:element  this={/* c */ tag} >text1   text2</svelte:element  >c</pre>

<!-- nested in an inline element, a component and another special element -->
<pre>a<b><svelte:element  this="b" >text1   text2</svelte:element  ></b>c</pre>

<pre>a<Comp><svelte:element  this="b" >text1   text2</svelte:element  ></Comp>c</pre>

<pre>a<svelte:component  this={Comp}
		 ><svelte:element  this="b" >text1   text2</svelte:element  ></svelte:component
	>c</pre>
