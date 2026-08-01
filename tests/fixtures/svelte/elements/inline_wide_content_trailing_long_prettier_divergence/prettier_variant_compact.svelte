<!--
	A wide inline element whose own content (not its attributes) overflows, followed by terminal
	trailing text. tsv wraps the content within printWidth (both tags intact) and hugs the trailing
	text onto the intact closing tag. That boundary is render-free, so the space- and
	newline-authored spellings converge on this one form; an authored BLANK line is a separate
	signal and survives (variant_blank_line_tail). Prettier keeps the content on one over-width
	line and distinguishes the two boundaries. See README. Covers <strong>/<a>.
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
