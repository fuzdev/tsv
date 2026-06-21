<!-- Fits: the inline span's line is exactly 100, so it stays on one line and the leading
     newline before the first child collapses to a single space (`<span> text1…`). -->
<p>
	<span> text1 text2 text3 text4 text5 text6 text7 text8 text9abcdefghij <a href="#">link</a></span>
</p>

<!-- Breaks: one char longer (101), so the span wraps multiline; the content text leads its own
     line and the parent's leading break supplies the boundary, so no stray leading space appears
     before the first word. -->
<p>
	<span>
		text1 text2 text3 text4 text5 text6 text7 text8 text9abcdefghijk <a href="#">link</a></span
	>
</p>
