<!--
	A wide inline element whose own content (not its attributes) overflows, followed by
	trailing text authored with a space boundary. tsv wraps the content within printWidth and
	hugs the trailing text onto the line of the dangled closing `>` (respecting the author's
	space); a newline-authored boundary instead keeps it on its own line (see the sibling
	inline_wide_content_trailing_newline_long). Prettier keeps the content on one over-width
	line. See README. Covers <strong>/<a>.
-->
<p>
	<strong
		>alpha bravo charlie delta echo foxtrot golf hotel india juliett kilo lima mike november oscar</strong
	> tail
</p>

<p>
	<a href="#"
		>november oscar papa quebec romeo sierra tango uniform victor whiskey xray yankee zulu abcdefg</a
	> tail
</p>
