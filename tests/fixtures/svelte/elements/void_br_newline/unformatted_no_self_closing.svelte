<!--
	A `<br />` is a rendered line break, so a source newline beside it mirrors the render: it is
	not a spelling of a space, and the sibling-newline flow rule holds it. Like a comment, the
	`<br />` owns its line — it BOUNDS a run without sterilizing it (the runs on either side are
	ordinary fills). Both formatters hold every authoring below, and a `<br />` never FORCES a
	break: the space and hugged spellings stay as authored.
-->

<!-- newline on both sides -->
<p>
	text1
	<br>
	text2
</p>

<!-- glued before, newline after — the line-per-line shape -->
<p>
	text1 text2<br>
	text3 text4<br>
	text5 text6
</p>

<!-- space before, newline after -->
<p>
	text1 text2 <br>
	text3
</p>

<!-- control: the space spelling stays as authored — a `<br />` never forces a break -->
<p>
	text1 <br> text2
</p>
<p>text1 <br> text2</p>

<!-- a `<br />` before an inline element and before a tag: the newline holds, and the run after
     it flows -->
<p>
	text1
	<br>
	<span>inline1</span> text2
	<br>
	{expr} text3
</p>

<!-- two in a row, each on its own line -->
<p>
	text1
	<br>
	<br>
	text2
</p>

<!-- after a unit that renders multiline -->
<p>
	text1
	<span>
		<b>inline1</b>
	</span>
	<br>
	text2
</p>

<!-- an inline container -->
<span>
	text1
	<br>
	text2
</span>
