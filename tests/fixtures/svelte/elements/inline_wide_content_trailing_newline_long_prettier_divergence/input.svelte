<!--
	A wide inline element whose own content overflows, followed by trailing text authored with a
	newline boundary. tsv lays the content out block-style (both tags intact, content wrapped on its
	own indented lines) and keeps the trailing text on its own line (the newline boundary). Prettier
	instead dangles the tags and keeps the content on one over-width line. The
	unformatted_ours_widecontent variant (content authored on one line) normalizes to this form
	under tsv but not prettier. See README. Covers <strong>/<a>.
-->
<p>
	<strong>
		alpha bravo charlie delta echo foxtrot golf hotel india juliett kilo lima mike november oscar
	</strong>
	tail
</p>

<p>
	<a href="#">
		november oscar papa quebec romeo sierra tango uniform victor whiskey xray yankee zulu abcdefg
	</a>
	tail
</p>
