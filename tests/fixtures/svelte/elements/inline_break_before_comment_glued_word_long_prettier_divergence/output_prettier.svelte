<!--
	A glued text word, a glued HTML comment, and an inline element form one glued run with
	no whitespace between them, so the break lands at the last whitespace boundary BEFORE
	the word: the whole run (word, comment, element) moves to the fresh line together.
	Breaking inside the run would inject a rendered space. Cases: fits at exactly 100 (stays
	inline, control); 101 (breaks before the glued word + collapses). Prettier pulls the
	glued word back onto the text line and dangles the element — see output_prettier.svelte
	(prettier keeps that form; tsv normalizes it to input).
-->
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 wdx glued<!--c--><a href="/a/b">content</a>.
</p>
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 wdxy glued<!--c--><a href="/a/b">content</a
	>.
</p>
