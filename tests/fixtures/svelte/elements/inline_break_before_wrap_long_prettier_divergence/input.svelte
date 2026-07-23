<!--
	An inline element preceded by same-line inline content that must wrap starts on a
	fresh line: tsv breaks at the whitespace boundary before the element rather than
	dangling its opening tag at the end of the text line. On its fresh line the element
	collapses back inline when it fits, else lays out block-style (both tags intact,
	content on its own line) or wraps its attributes. Cases below, in order: fits at
	exactly 100 (stays inline, control); 101 (breaks + collapses); last child (collapses);
	content too long to collapse (block-style); opening tag itself over 100 (attrs wrap).
	Prettier keeps the opening tag on the text line and dangles it — see
	prettier_variant_dangle.svelte (prettier keeps that form; tsv normalizes it to input).
-->
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 ab <a href="/a/b">content</a>.
</p>
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 abc
	<a href="/a/b">content</a>.
</p>
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12
	<a href="/a/b">content</a>
</p>
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12
	<a href="/a">
		content1 content2 content3 content4 content5 content6 content7 content8 content9 content10
	</a>.
</p>
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12
	<a
		href="/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	>
		content
	</a>.
</p>
