<!-- A wide inline element whose own content (not its attributes) overflows, followed by
     trailing text authored with a newline boundary. tsv wraps the content within printWidth and
     keeps the trailing text on its own line — respecting the author's newline; a space-authored
     boundary instead hugs the dangled closing `>` (see the sibling
     inline_wide_content_trailing_long). Prettier keeps the content on one over-width line. See
     README. Covers <strong>/<a>. -->
<p>
	<strong
		>alpha bravo charlie delta echo foxtrot golf hotel india juliett kilo lima mike november oscar</strong
	>
	tail
</p>

<p>
	<a href="#"
		>november oscar papa quebec romeo sierra tango uniform victor whiskey xray yankee zulu abcdefg</a
	>
	tail
</p>
