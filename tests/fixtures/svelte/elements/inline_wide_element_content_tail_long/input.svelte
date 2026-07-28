<!--
	A wide inline element whose content is a nested inline ELEMENT (not text), followed by a
	NON-TERMINAL text run — another inline element follows it. The element lays out block-style
	with both tags intact, and the text run takes its own line: non-terminal text must not hug the
	closing tag, since that shifts where the following element lands (the guard
	inline_wide_content_text_sibling_long states). Both formatters agree, so this is not a
	divergence; what it pins is that the form is a FIXED POINT — the space-authored boundary must
	settle in one pass, not hug on a second.
-->

<!-- 100 chars - the element fits inline, so it collapses and the text run follows on its own line -->
<p>
	<span data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"><code>text1</code></span>
	text2 <span>text3</span>, text4
</p>

<!-- 101 chars - the element breaks block-style; the non-terminal text run keeps its own line -->
<p>
	<span data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">
		<code>text1</code>
	</span>
	text2 <span>text3</span>, text4
</p>
