<!--
	Glued prefix travels with the element. When an inline element is glued (no whitespace)
	to preceding text/inline content, the break lands at the last whitespace boundary before
	the whole glued run, which moves to the fresh line together — never between the glued
	text and the element (that would inject a rendered space). And the unit is measured to
	its own END and no further: a spaced follower (` mid <b>x</b>`) wraps behind the unit's
	own trailing boundary, which never enters the unit's fit check. Cases: the glued run
	travels whole (terminal `.` glued to the element); a spaced non-terminal follower with
	the unit ending at exactly 100 (the unit packs onto the text line and only the follower
	wraps); the same at 101 (the unit travels and the follower packs after it). Prettier
	keeps each glued run on the text line and dangles the tag delimiters — see
	output_prettier.svelte.
-->
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11
	glued<a href="/a/b">content</a>.
</p>
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 abcde glued<a href="/a/b">content</a>
	mid <b>x</b>
</p>
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 abcdef
	glued<a href="/a/b">content</a> mid <b>x</b>
</p>
