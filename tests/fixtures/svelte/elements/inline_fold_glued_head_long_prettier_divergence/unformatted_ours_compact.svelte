<!--
	A welded run — inline element, glued text, inline element — with terminal trailing text
	after it. The glued boundaries are never split (breaking one injects a rendered space),
	but the run is still MOVABLE: it travels whole to the whitespace boundary in front of it.
	Cases: whole line @100 (all inline); the run still fits after its preceding word, line
	@100 (only the terminal tail drops); @101 (the whole run travels to a fresh line
	together); and the spaced control at that same column, where the boundary sits INSIDE the
	run, so only the part after it moves. Prettier keeps the run on the text line and dangles
	the closing `>` — see prettier_variant_dangle.svelte.
-->
<p>text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 <code>.x</code>.w<b>yyyyyyyy</b> tail</p>
<p>text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 aaaa <code>.x</code>.w<b>yyyyyyyy</b> tail</p>
<p>text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 aaaaa <code>.x</code>.w<b>yyyyyyyy</b> tail</p>
<p>text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 aaaa <code>.x</code>.w <b>yyyyyyyy</b> tail</p>
