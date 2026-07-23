<!--
	Glued inline-element chain (G2). When an inline element is glued (no whitespace) to a
	following inline element whose own opening tag must wrap, tsv dangles the FIRST element's
	closing `>` onto the second element's line (`</span` on one line, `><a` on the next) so no
	opening tag is stranded at a line end after a space. The whole glued run first breaks to a
	fresh line at the whitespace boundary before it. The `>` move is tag-internal (byte-identical
	AST), so it is render-safe. Prettier keeps the run on the text line and dangles the second
	tag's attributes instead — see output_prettier.svelte.
-->
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12 <span>foo</span><a
		href="/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
		>content</a
	>.
</p>
