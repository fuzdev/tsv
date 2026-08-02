<!--
	The welded word+tag pair travels. A `{expr}` tag glued to the end of a text word is the
	SMALLEST welded unit: the word and its tag are measured together, and when the pair does
	not fit, the break lands at the whitespace boundary in front of the word — the pair moves
	to the fresh line together, holding printWidth as a hard limit. Prettier keeps the tag
	OUTSIDE the text fill, so its fill never sees the tag's width: the word stays put and the
	tag rides past printWidth after it — see output_prettier.svelte. Cases, probed at the
	exact 100/101 boundary: the pair at exactly 100 packs (both formatters keep it); at 101
	the pair travels; the same with a spaced follower (` tail1 tail2`), which packs after the
	pair on the fresh line; and a tag whose expression itself must break — the pair still
	travels first, and the expression breaks internally on the fresh line rather than opening
	mid-line.
-->
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 aaaaaaaaaaaaaaaaa glued{expr12345678}
</p>
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 aaaaaaaaaaaaaaaaaa
	glued{expr12345678}
</p>
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 aaaaaaaaaaaaaaaaaa
	glued{expr12345678} tail1 tail2
</p>
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10
	glued{cond1 === cond2
		? 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
		: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'}
</p>
