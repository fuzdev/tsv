<!-- literal NBSP before an inline element: glues "word" to <code>, no separating space -->
<p>
	text1 word <code>.x</code> text2
</p>

<!-- literal NBSP between two inline elements: glues "word" to <b>, no space -->
<p>
	<code>.x</code> word <b>.y</b> text3
</p>

<!-- &nbsp; entity: glues the same as a literal NBSP -->
<p>
	text1 word&nbsp;<code>.x</code> text2
</p>

<!-- regular space: ordinary breakable space before the element -->
<p>
	text1 word <code>.x</code> text2
</p>
