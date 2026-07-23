<!--
	An inline component preceded by same-line text breaks to a fresh line and collapses
	inline, rather than dangling its opening tag on the text line. Prettier keeps the opening
	tag on the text line and dangles it — see prettier_variant_dangle.svelte.
-->
<p>text1 text2 text3 text4 text5 text6 <Comp href="https://fuz.dev/long/path/to/resource/aaaa">MDN</Comp>.</p>
