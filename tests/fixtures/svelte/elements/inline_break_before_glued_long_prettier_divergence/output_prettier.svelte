<!--
	Glued prefix travels with the element. When an inline element is glued (no whitespace)
	to preceding text/inline content, the break lands at the last whitespace boundary before
	the whole glued run, which moves to the fresh line together — never between the glued
	text and the element (that would inject a rendered space). And the unit is measured to
	its own END and no further: a spaced follower (` mid <b>x</b>`) wraps behind the unit's
	own trailing boundary, which never enters the unit's fit check. Cases: the glued run
	travels whole (terminal `.` glued to the element); a spaced non-terminal follower with
	the unit ending at exactly 100 (the unit packs onto the text line and only the follower
	wraps); the same at 101 (the unit travels and the follower packs after it); and the glue
	running THROUGH an expression tag (`glued{x}<a …>`): a tag welded onward into an element
	travels with the run all the same — one case crossing past the tag (inside the element)
	and one whose tag is itself the crossing point; and a glued tag whose expression must
	break: the unit still travels first, and the expression then breaks internally on the
	fresh line (the wide-element rule's tag analog — content that cannot fit flat starts on
	a fresh line rather than opening mid-line). (A glued tag that ENDS the run travels the
	same way — the word+tag pair is the smallest welded unit; fill_glued_tag_travel_long
	pins that contract.) Two more follower axes, each probed at the exact 100/101 boundary:
	the tag welded onward into ANOTHER tag (`glued{x}{y}` — mid-run glue, the measurement
	walks through the first tag into the second, so 100 packs and 101 travels), and the tag
	glued to a following BLOCK element — glue that survives only in the source, since the
	block detaches to its own line regardless — where the measured unit is therefore the
	word+tag pair alone: at exactly 100 it packs, at 101 the pair travels. Prettier keeps
	each glued run on the text line and dangles the tag delimiters — see
	output_prettier.svelte.
-->
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 glued<a href="/a/b">content</a
	>.
</p>
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 abcde glued<a href="/a/b">content</a>
	mid <b>x</b>
</p>
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 abcdef glued<a href="/a/b">content</a
	>
	mid <b>x</b>
</p>
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 aaaaaaaaaaaaaaaaaa glued{x}<a
		href="/a/b">content</a
	>.
</p>
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 aaaaaaaaaaaaaaaaaa glued{expr12345678}<a
		href="/a/b">c</a
	>
</p>
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 glued{cond1 === cond2
		? 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
		: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'})
</p>
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 aaaaaaaaaaa glued{expr1234}{expr5678}
</p>
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 aaaaaaaaaaaa glued{expr1234}{expr5678}
</p>
<div>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 aaaaaaaaaaaaaaaaa glued{expr12345678}
	<div data-attr="value">block1</div>
</div>
<div>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 aaaaaaaaaaaaaaaaaa glued{expr12345678}
	<div data-attr="value">block1</div>
</div>
