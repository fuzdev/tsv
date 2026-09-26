<!--
	Inside `<pre>` a special element's content is rendered text and cannot break, so a line too wide
	for it breaks inside the element's own tags: the `>` takes the next line and the closing tag
	dangles its `>`, the shape a component takes there.
-->

<!-- exactly 100 columns: nothing moves -->
<pre>a<svelte:element this="b">aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa text1   text2</svelte:element>c</pre>

<!-- 101 columns: the closing `>` dangles -->
<pre>a<svelte:element this="b">aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa text1   text2</svelte:element>c</pre>

<!-- the same boundary for `<svelte:component>` -->
<pre>a<svelte:component this={Comp}>aaaaaaaaaaaaaaaaaaaaaaaa text1   text2</svelte:component>c</pre>

<pre>a<svelte:component this={Comp}>aaaaaaaaaaaaaaaaaaaaaaaaa text1   text2</svelte:component>c</pre>

<!-- an expression among the content is no break point: the `>` takes the next line -->
<pre>a<svelte:element this="b">aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa text1  {expr}  text2 aaaaaaaaaaaaaaaaaaaa</svelte:element>c</pre>

<!-- content opening and closing on whitespace: neither delimiter moves, so the list wraps and the
	line still runs over -->
<pre>a<svelte:element this="b"> aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa text1   text2 aaaaaaaaaaaaaaaaaaaa </svelte:element>c</pre>

<!-- a long attribute list wraps one attribute per line and the `>` hugs the last one -->
<pre>a<svelte:element this="b" data-attr1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" data-attr2="value2">text1   text2</svelte:element>c</pre>
