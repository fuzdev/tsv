<!--
	A frozen node inside `<pre>` is as wide as its source text. Beside it an unfrozen element fits
	at 100 characters and dangles its closing `>` at 101, the break landing inside the tag.
-->

<!-- 100 characters: fits -->
<pre>a<!-- prettier-ignore --><b   data-a = "x" >t</b><b data-a="yyyyyyyyyyyyyyyyyyyyy">t</b>c</pre>

<!-- 101 characters: the unfrozen element's closing `>` breaks -->
<pre>a<!-- prettier-ignore --><b   data-a = "x" >t</b><b data-a="yyyyyyyyyyyyyyyyyyyyyy">t</b
	>c</pre>

<!-- a frozen node 107 characters wide on its own is never broken; its unfrozen twin breaks -->
<pre>a<!-- prettier-ignore --><b   data-a = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"   data-b = "zzzzzzzzzzzzzzzzzzzz" >t</b>c</pre>

<pre>a<b data-a="xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx" data-b="zzzzzzzzzzzzzzzzzzzz"
		>t</b
	>c</pre>
