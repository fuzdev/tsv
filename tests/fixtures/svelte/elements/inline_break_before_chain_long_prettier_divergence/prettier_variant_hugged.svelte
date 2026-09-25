<!--
	Glued inline-element CHAIN of 3+ (G2, generalized to a run). A whole run of byte-glued
	inline elements travels together: preceded by same-line text that must wrap, the entire
	run breaks to a fresh line at the whitespace boundary before it (a later wide element pulls
	the whole run over — no earlier short element can strand the run on the text line), and
	at each glued boundary the first element's closing `>` hugs the next tag while its attributes
	fit and drops onto its line when they wrap (`</span><b`, then `</b` / `><a`). Prettier keeps the
	run on the text line and dangles the last tag's attributes instead — see output_prettier.svelte.
-->
<p>
	word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 <span>a</span><b>b</b><a
		href="/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
		>content</a
	>.
</p>
