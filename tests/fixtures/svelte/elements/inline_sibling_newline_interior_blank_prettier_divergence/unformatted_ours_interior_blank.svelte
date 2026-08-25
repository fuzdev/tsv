<!--
	An authored blank line INSIDE one text node (`text1 text2⏎⏎text3 text4`, one Text node) is
	not a run boundary: the flow rule's scan partitions NODES and can bound a run only at a
	blank on a text's EDGE, while the fill collapses the interior blank either way — under
	both formatters. So the sibling newlines beside such a node flow with the run's prose,
	where prettier holds them.
-->

<!-- a two-word half on each side of the blank, beside inline siblings -->
<p>
	<span>inline1</span>
	text1 text2

	text3 text4
	<span>inline2</span>
</p>
