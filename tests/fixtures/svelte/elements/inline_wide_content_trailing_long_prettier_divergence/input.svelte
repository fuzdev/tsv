<!-- A wide inline element whose own content (not its attributes) overflows, followed by
     trailing text. tsv wraps the content within printWidth and puts the trailing text on its
     own line — it never hugs trailing text onto a wrapped element's last line. Prettier keeps
     the content on one over-width line; its tail placement is authoring-dependent (hug vs
     own-line — see prettier_variant_hug). See README. Covers <strong>/<a>. -->
<p>
	<strong
		>alpha bravo charlie delta echo foxtrot golf hotel india juliett kilo lima mike november
		oscar</strong
	>
	tail
</p>

<p>
	<a href="#"
		>november oscar papa quebec romeo sierra tango uniform victor whiskey xray yankee zulu
		abcdefg</a
	>
	tail
</p>
