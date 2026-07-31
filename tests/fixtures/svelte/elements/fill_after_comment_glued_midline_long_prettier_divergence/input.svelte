<!--
	A text run byte-glued to a preceding HTML comment travels WITH it: that boundary carries no
	whitespace, so the only break point is the one in FRONT of the comment. Once the glued unit no
	longer fits where it sits, the whole unit moves to a fresh line — the space ahead of it is
	inter-node whitespace that collapses to one rendered space, so tsv is free to spend it on the
	break. Cases below, in order: the unit still fits mid-line (control, no move); the unit
	overflows mid-line at 104 but fits at line start (it moves, and now fits at exactly 100); the
	unit is wider than a whole line at 101 (it moves anyway — the break is spent, the limit still
	is not met); the boundary in FRONT of the unit is itself glued, so the unit cannot move at all
	and the break travels further left, to the space inside the preceding run. Prettier welds the
	unit to the preceding word and lets the line run over — see output_prettier.svelte.
-->
<p>
	foo <!--pad1 pad2 pad3 pad4 pad5 pad6 pad7 pad8 pad9 pad10 pad11 pad12 pad13 pad14 pad15-->text1
	text2
</p>
<p>
	foo
	<!--pad1 pad2 pad3 pad4 pad5 pad6 pad7 pad8 pad9 pad10 pad11 pad12 pad13 pad14 pad15 pad16-->text1
	text2
</p>
<p>
	foo
	<!--pad1 pad2 pad3 pad4 pad5 pad6 pad7 pad8 pad9 pad10 pad11 pad12 pad13 pad14 pad15 pad16x-->text1
	text2
</p>
<p>
	foo
	0<!--pad1 pad2 pad3 pad4 pad5 pad6 pad7 pad8 pad9 pad10 pad11 pad12 pad13 pad14 pad15 pad16-->text1
	text2
</p>
