<!--
	The sibling-newline flow rule's prose gate is a HOLD on an authored newline, never a forced
	one: a one-word run keeps its authored lines, but its SPACE-spelled tag pair is a fill like a
	prose run's and packs per width, where prettier's bare `line` between two tags breaks with
	the multiline container. Only a prose-FREE run's tag pair — no content text at all — takes
	that break under tsv too: the control.
-->

<!-- a word before a spaced tag pair, and a word after one -->
<p>
	text1 {expr1}
	{expr2}
</p>
<p>
	{expr1}
	{expr2}
	{expr3} text1
</p>

<!-- at the root, which is always a multiline container: the escaped-angle-bracket pair, whose
     glued `div` / `code` words make the run a one-word run -->
{'<'}div{'>'}
{'<'}code{'>'}{'<'}/code{'>'}

<!-- control: a prose-free pair has no fill at all, and its space breaks with the container
     under both formatters -->
<p>
	{expr1}
	{expr2}
</p>
