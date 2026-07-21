<!--
	A wide inline element whose own content overflows, SANDWICHED (a text sibling precedes it)
	and followed by TERMINAL trailing text with a space boundary. tsv wraps the content within
	printWidth (both tags intact) and the space-authored terminal text hugs the closing tag
	(`</tag> tail`) — a preceding sibling does not change that terminal trailing text hugs, since
	nothing follows it (the first-child case is inline_wide_content_trailing_long). Prettier keeps
	this block-style form, but a single-line authoring it dangles instead (prettier_variant_compact).
	Covers <strong>/<a>.
-->
<p>
	text1 text2
	<strong>
		alpha bravo charlie delta echo foxtrot golf hotel india juliett kilo lima mike november oscar
	</strong> tail
</p>

<p>
	text1 text2
	<a href="#">
		november oscar papa quebec romeo sierra tango uniform victor whiskey xray yankee zulu abcdefg
	</a> tail
</p>
