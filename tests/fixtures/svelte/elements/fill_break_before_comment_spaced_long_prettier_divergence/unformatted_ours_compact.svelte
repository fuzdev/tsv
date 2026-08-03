<!--
	An HTML comment spaced on BOTH sides inside a text fill keeps the break point in FRONT of it:
	that boundary is inter-node whitespace and collapses to one rendered space, so turning it into
	a line break is render-equivalent. The comment is its own fill item, so the word before it is
	measured without it and the comment starts a fresh line once it no longer fits. Cases below, in
	order: the comment ends the line at exactly 100 (it stays, control); 101 (the comment moves to
	its own line and the preceding word keeps its column); a comment that cannot share a line with
	the preceding word at any column (the break in front of it is the only layout within
	printWidth); that same boundary with nothing after the comment at all; and the same boundary in
	an INLINE container and in a table cell, whose content is collapsible so the container carries
	no multiline cause — the boundary is the same question there. Prettier packs the comment onto
	the text line and lets the line run over — see unformatted_ours_compact.svelte.
-->
<p>
	text1 text2 text3 text4 text5 text6 text7 <!--pad1 pad2 pad3 pad4 pad5 pad6 pad7 pad8 pad9xxxxx--> text8 text9
</p>
<p>
	text1 text2 text3 text4 text5 text6 text7 <!--pad1 pad2 pad3 pad4 pad5 pad6 pad7 pad8 pad9xxxxxx--> text8 text9
</p>
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx <!--pad1 pad2 pad3 pad4 pad5 pad6 pad7 pad8 pad9--> text9 text10
</p>
<p>
	text1 text2 text3 text4 text5 text6 text7 <!--pad1 pad2 pad3 pad4 pad5 pad6 pad7 pad8 pad9xxxxxx-->
</p>
<span>text1 text2 text3 text4 text5 text6 text7 text8 xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx <!--pad1 pad2 pad3 pad4 pad5 pad6 pad7 pad8 pad9--> text9 text10</span>
<table><tr><td>text1 text2 text3 text4 text5 text6 text7 text8 xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx <!--pad1 pad2 pad3 pad4 pad5 pad6 pad7 pad8 pad9--> text9 text10</td></tr></table>
