<!--
	A spaced {expr} tag whose expression CAN break takes the same whole-flat pairwise
	measurement as every other flow follower: when the tag does not fit FLAT after the last
	word, the boundary in front of it breaks and the tag starts on a fresh line — collapsing
	flat there when it fits, breaking internally there when not — rather than opening
	mid-line. Prettier's measurement stops at the expression's first internal break, so it
	keeps the tag on the text line and opens it mid-line (see prettier_variant_midline).
	Cases at the exact boundary: at 100 the tag packs flat mid-line (both formatters keep
	it); at 101 it travels and collapses flat on the fresh line; a far wider expression
	still collapses flat on the fresh line when it fits there; and one too wide even for
	the fresh line travels first, then breaks internally.
-->
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 aaa {cond1 === cond2 ? 'aaa' : 'bbb'}
</p>
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 aaaa {cond1 === cond2
		? 'aaa'
		: 'bbb'}
</p>
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 spaced {cond1 === cond2
		? 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
		: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'}
</p>
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 spaced {cond1 === cond2
		? 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
		: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'}
</p>
