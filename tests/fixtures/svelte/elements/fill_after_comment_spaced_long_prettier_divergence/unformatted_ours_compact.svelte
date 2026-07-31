<!--
	A text run separated from a preceding HTML comment by a SPACE keeps a break point there:
	that boundary is inter-node whitespace and collapses to one rendered space, so turning it
	into a line break is render-equivalent. Once the run's first word no longer fits after the
	comment, the whole run starts a fresh line and flows there as one fill — the boundary space
	is spent on the break, never re-emitted at the head of the continuation line. Cases below,
	in order: the whole line fits at exactly 100 (stays inline, control); the first word fits at
	exactly 100 (it stays on the comment line, the rest wraps); 101 (the run moves to its own
	line); a longer run that wraps again on its fresh line. Prettier keeps the first word on the
	comment line and lets the line run past printWidth — see output_prettier.svelte.
-->
<p>
	<!--pad1 pad2 pad3 pad4 pad5 pad6 pad7 pad8 pad9 pad10 pad11 pad12 pad13 pad14xxxxx--> text1 text2
</p>
<p>
	<!--pad1 pad2 pad3 pad4 pad5 pad6 pad7 pad8 pad9 pad10 pad11 pad12 pad13 pad14 pad15xxxxx--> text1 text2
</p>
<p>
	<!--pad1 pad2 pad3 pad4 pad5 pad6 pad7 pad8 pad9 pad10 pad11 pad12 pad13 pad14 pad15 pad16--> text1 text2
</p>
<p>
	<!--pad1 pad2 pad3 pad4 pad5 pad6 pad7 pad8 pad9 pad10 pad11 pad12 pad13 pad14 pad15 pad16--> text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 text13 text14 text15 text16
</p>
