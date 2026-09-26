<!--
	Text in front of a glued nested `<script>`: the glued run renders multiline, so the space in
	front of it is spent on a line break at every width and the run starts its own line; the glued
	boundaries stay intact.
-->

<!-- the text line with the run would end at exactly 100 -->
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 text13 text14
	a<script>
		let a = 1;
	</script>text15
</p>

<!-- one character wider (101) -->
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 text13 text14
	aa<script>
		let b = 2;
	</script>text15
</p>
