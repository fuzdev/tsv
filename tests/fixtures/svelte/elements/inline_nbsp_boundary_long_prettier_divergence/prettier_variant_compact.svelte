<!-- literal non-breaking space, exactly 100: glues "te" to </code>, stays inline -->
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 aaaa <code>.x</code> te
</p>

<!-- literal non-breaking space, exactly 101: "tex" tips past printWidth, so <code> lays out block-style (.x on its own line); the NBSP keeps "tex" glued to the </code> line -->
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 aaaa <code>.x</code
	> tex
</p>

<!-- &nbsp; entity, exactly 100: glues "te" to </code>, stays inline -->
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 <code>.x</code>&nbsp;te
</p>

<!-- &nbsp; entity, exactly 101: "tex" tips past printWidth, so <code> lays out block-style (.x on its own line); the &nbsp; keeps "tex" glued to the </code> line -->
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 <code>.x</code
	>&nbsp;tex
</p>
