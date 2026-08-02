<!--
	A welded run travels to the whitespace boundary in front of it whatever the run is made of,
	whatever ends it, and whatever follows it. The rule is the unit's, not the trailing node's.
	Cases: no terminal tail, line @100 (fits, stays); @101 (the run travels); the same run
	followed by non-terminal text and a further element (the run travels the same way and the
	follower packs after it); a three-element run whose MIDDLE element is the one that no longer
	fits (the whole run still travels as one); a three-element run whose LAST element is the one
	that no longer fits while every earlier member still fits (no earlier short element strands
	a later wide one); an `&nbsp;`-glued run (content glue, never collapses); a component pair;
	an expression tag ending the run; a tag-HEADED run at line @100 (fits, stays) and @101 (the
	run travels — the crossing point sits past the tag, in the last element); a tag-headed run
	whose TAG is the crossing point (the run travels the same way — which side of the tag the
	width runs out on cannot matter); and a run welded THROUGH a mid-run tag
	(element-tag-element glue, crossing past the tag). Prettier keeps each run on the text line
	and dangles the tag delimiters — see prettier_variant_dangle.svelte.
-->
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 aaaa <code>.x</code>.w<b>yyyyyyyy</b>
</p>
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 aaaaa <code>.x</code>.w<b>yyyyyyyy</b
	>
</p>
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 aaaaa <code>.x</code>.w<b>yyyyyyyy</b
	>
	mid <i>zz</i>
</p>
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 aaaaaaaaaaaaaaaaaaaa <code>.x</code>.w<b
		>yy</b
	>.z<i>qq</i> tail
</p>
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 aaaaaaaa <code>.x</code>.w<b>yy</b>.z<i
		>qqqqqqqq</i
	> tail
</p>
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 aaaa <code>.x</code>&nbsp;<b
		>yyyyyyyy</b
	> tail
</p>
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 aaaaaaaaaaaaaa <Comp1 />.w<Comp2
		a="v"
	/> tail
</p>
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 aaaaaaaaaaaaaaaaaaaa <code>.x</code
	>.w{expr} tail
</p>
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 aaaa {expr123456789}.w<b>yyyyyyyy</b>
</p>
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 aaaaa {expr123456789}.w<b>yyyyyyyy</b
	>
</p>
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 aaaaaaaaaaaaaaaaaa {expr12345678901234}.w<b
		>yy</b
	>
</p>
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 aaa <code>.x</code>{ex}<b>yyyyyyyy</b
	>
</p>
