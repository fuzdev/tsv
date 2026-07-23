<!--
	Component parity for the break-before rule: an inline component preceded by same-line
	text starts on a fresh line rather than dangling its opening tag. It collapses back
	inline when it fits, else lays out block-style. Byte-identical in shape to the HTML
	inline-element fixture (inline_break_before_wrap_long) — the fold is element-type
	agnostic. Prettier keeps the opening tag on the text line and dangles it — see
	prettier_variant_dangle.svelte.
-->
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12
	<Comp href="/a/b">content</Comp>.
</p>
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12
	<Comp href="/a">
		content1 content2 content3 content4 content5 content6 content7 content8 content9 content10
	</Comp>.
</p>
