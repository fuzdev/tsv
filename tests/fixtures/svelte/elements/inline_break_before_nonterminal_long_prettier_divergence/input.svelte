<!--
	The break-before rule with a following whitespace-separated sibling (non-terminal): the
	element breaks to a fresh line and lays out block-style (content too long to collapse); the
	terminal trailing text ` tail` hugs the intact `</a>`. Prettier keeps the opening tag on the
	text line and dangles it — see prettier_variant_dangle.svelte.
-->
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12
	<a href="/a">
		content1 content2 content3 content4 content5 content6 content7 content8 content9 content10
	</a> tail
</p>
