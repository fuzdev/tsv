<!--
	Text after a glued nested `<script>` fills from the closing tag's line: the glued boundaries
	stay intact, and the text wraps at print width like any other fill.
-->

<!-- the line after the closing tag is exactly 100 wide: it stays on one line -->
<p>
	text1<script>
		let a = 1;
	</script>text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 text13 text14 tttttt
</p>

<!-- one character wider (101): the last word wraps -->
<p>
	text1<script>
		let b = 2;
	</script>text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 text13 text14
	ttttttt
</p>
