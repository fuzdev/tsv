<!--
	A `format-ignore` inside `<pre>` freezes the next node, as a `prettier-ignore` there does.
-->

<!-- directly in `<pre>`, beside a `prettier-ignore` control -->
<pre>a<!-- format-ignore --><b a="1">text1</b>c</pre>

<pre>a<!-- prettier-ignore --><b   a = "1" >text2</b>c</pre>

<!-- inside a block section, an element and a special element there -->
<pre>{#if cond}<!-- format-ignore -->{expr}{/if}</pre>

<pre>a<i><!-- format-ignore --><Comp a="1" /></i>c</pre>

<pre><svelte:element this="i"><!-- format-ignore --><b a="1">text3</b></svelte:element></pre>
