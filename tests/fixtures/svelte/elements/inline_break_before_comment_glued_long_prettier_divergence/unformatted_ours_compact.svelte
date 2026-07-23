<!--
	An inline element preceded by same-line content and a GLUED HTML comment breaks
	before the comment: the comment is the element's glued prefix and travels with it to
	the fresh line, exactly as a glued text prefix does. The break lands at the last
	whitespace boundary before the comment (never between the comment and the element —
	that boundary is glued). On its fresh line the element lays out like any other:
	collapses inline when it fits, else block-style. Cases below, in order: fits at exactly
	100 (stays inline, control); 101 (breaks + collapses); content too long to collapse
	(block-style); a run of two glued comments (both travel together). Prettier keeps the
	opening tag on the text line and dangles it — see prettier_variant_dangle.svelte
	(prettier keeps that form; tsv normalizes it to input).
-->
<p>word1 word2 word3 word4 word5 word6 word7 word8 word9 word10ab <!--c--><a href="/a/b">content</a>.</p>
<p>word1 word2 word3 word4 word5 word6 word7 word8 word9 word10abc <!--c--><a href="/a/b">content</a>.</p>
<p>word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12 <!--c--><a href="/a">content1 content2 content3 content4 content5 content6 content7 content8 content9 content10</a>.</p>
<p>word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12 <!--a--><!--b--><a href="/a/b">content</a>.</p>
