<!-- 100: the component renders inline (its line is exactly 100), so the authored
	newline after it is a spelling the fill reflows — text3 takes its own line by width. -->
<p>
	text1
	<Comp prop="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">text2</Comp>
	text3
</p>

<!-- 101: one character wider, the component renders multiline (block-style), and the
	authored newline after it is PRESERVED — text6 keeps its own line where a space
	would hug (variant_welded). -->
<p>
	text4
	<Comp prop="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">
		text5
	</Comp> text6
</p>
