<!--
	A wide inline element whose content is a nested inline ELEMENT (not text), followed by a
	NON-TERMINAL text run — another inline element follows it. The tail boundary after such an
	element is a per-width fill decision measured from the closing tag's own column: it hugs the
	intact closing tag when the tail fits and breaks when it does not, converging the hugged,
	spaced and newline authorings onto one fixed point. Prettier instead groups the boundary WITH
	the element, so once the element is multiline the tail always drops — it keeps the own-line
	form stable (output_prettier) and re-breaks the hugged one.
-->

<!-- 100 chars - the element fits inline and collapses; the tail cannot join its exactly-100 line -->
<p>
	<span data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"><code>text1</code></span>
	text2 <span>text3</span>, text4
</p>

<!-- 101 chars - the element breaks block-style; the non-terminal tail hugs the intact closing tag -->
<p>
	<span data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"><code>text1</code></span
	>
	text2 <span>text3</span>, text4
</p>
