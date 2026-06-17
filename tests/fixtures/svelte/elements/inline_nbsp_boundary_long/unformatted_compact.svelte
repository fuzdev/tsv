<!-- literal non-breaking space, exactly 100: glues "te" to </code>, stays inline -->
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 aaaa <code>.x</code> te
</p>

<!-- literal non-breaking space, exactly 101: "tex" tips past printWidth; the NBSP stays glued, so the break is the closing > (hug), never the space -->
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 aaaa <code>.x</code> tex
</p>

<!-- &nbsp; entity, exactly 100: glues "te" to </code>, stays inline -->
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 <code>.x</code>&nbsp;te
</p>

<!-- &nbsp; entity, exactly 101: "tex" tips past printWidth; the entity stays glued, so the break is the closing > (hug), never the entity -->
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 <code>.x</code>&nbsp;tex
</p>
