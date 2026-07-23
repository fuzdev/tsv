<!--
	Glued prefix travels with the element. When an inline element is glued (no whitespace)
	to preceding text/inline content, the break lands at the last whitespace boundary before
	the whole glued run, which moves to the fresh line together — never between the glued
	text and the element (that would inject a rendered space). Here `glued` is glued to the
	<a>; the break is before `glued`. Prettier keeps the opening tag on the text line and
	dangles it — see prettier_variant_dangle.svelte.
-->
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11
	glued<a href="/a/b">content</a>.
</p>
