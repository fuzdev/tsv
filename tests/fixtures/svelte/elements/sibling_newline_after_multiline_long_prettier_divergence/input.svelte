<!-- 100: the component renders inline (its line is exactly 100), so the authored newline
	after it is a spelling the fill reflows — the <b> takes its own line by width alone. -->
<p>
	text1
	<Comp prop="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">text2</Comp>
	<b>inline1</b>
</p>

<!-- 101: one character wider, the component renders multiline (block-style), and the
	authored newline after it is PRESERVED — the <b> keeps its own line where a space
	would hug (variant_welded). -->
<p>
	text3
	<Comp prop="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">
		text4
	</Comp>
	<b>inline2</b>
</p>
