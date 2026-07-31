<!--
	A text run byte-glued to a preceding HTML comment is one indivisible run: breaking at
	that boundary would inject a rendered space, so the break can only land at a
	whitespace boundary INSIDE the text run — even when the glued head overflows
	printWidth. Cases below, in order: fits at exactly 100 (stays inline, control); 101
	(breaks at the space inside the run, glue intact); the glued head alone over width
	(101, stands); a run of two glued comments (the whole run stays welded).
-->
<p>
	<!--pad1 pad2 pad3 pad4 pad5 pad6 pad7 pad8 pad9 pad10 pad11 pad12 pad13 pad14 pad15-->text1 text2
</p>
<p>
	<!--pad1 pad2 pad3 pad4 pad5 pad6 pad7 pad8 pad9 pad10 pad11 pad12 pad13 pad14 pad15x-->text1
	text2
</p>
<p>
	<!--pad1 pad2 pad3 pad4 pad5 pad6 pad7 pad8 pad9 pad10 pad11 pad12 pad13 pad14 pad15 pad16x-->text1
	text2
</p>
<p>
	<!--a--><!--pad1 pad2 pad3 pad4 pad5 pad6 pad7 pad8 pad9 pad10 pad11 pad12 pad13 pad14-->text1
	text2
</p>
