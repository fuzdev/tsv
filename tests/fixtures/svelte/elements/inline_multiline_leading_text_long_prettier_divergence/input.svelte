<!-- Fits: the inline span's line is exactly 100, so it stays on one line and the leading
     newline before the first child collapses to a single space (`<span> text1…`). -->
<p>
	<span> text1 text2 text3 text4 text5 text6 text7 text8 text9abcdefghij <a href="#">link</a></span>
</p>

<!-- Breaks: one char longer (101), so the span lays out block-style — both tags intact, content on
     its own indented line. The content ends by hugging `</span>` (an element child, no boundary
     whitespace), but that hug is render-free and so does not select the layout; prettier lets it
     dangle the closing `>` instead — see prettier_variant_dangle. -->
<p>
	<span>
		text1 text2 text3 text4 text5 text6 text7 text8 text9abcdefghijk <a href="#">link</a>
	</span>
</p>
