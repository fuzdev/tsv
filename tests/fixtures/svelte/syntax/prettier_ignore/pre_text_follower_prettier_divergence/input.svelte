<!--
	A `prettier-ignore` followed by text freezes that text, and the node after it formats — inside
	`<pre>` as everywhere else.
-->

<!-- directly in `<pre>`, and inside an element and a special element there -->
<pre>a<!-- prettier-ignore -->text1<b a="1">text2</b>c</pre>

<pre>a<i><!-- prettier-ignore -->text1<b a="1">text2</b></i>c</pre>

<pre><svelte:element this="i"><!-- prettier-ignore -->text1<b a="1">text2</b></svelte:element></pre>

<!-- inside a block section -->
<pre>{#if cond}<!-- prettier-ignore -->text1<b a="1">text2</b>{/if}</pre>

<!-- outside `<pre>` -->
<p>a<!-- prettier-ignore -->text1<b a="1">text2</b>c</p>
