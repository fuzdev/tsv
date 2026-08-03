<!--
	Whitespace-sensitive: inside <pre> every inter-tag byte is content, so tsv's layout
	vocabulary (whitespace-boundary breaks, block-style moves, tags intact) has no
	render-safe break point and the over-width welded run stands. Prettier instead
	dangles the closing tag's `>` (`</b⏎\t>tail` — the break lands inside tag syntax,
	render-free) to duck under printWidth.
-->
<pre>
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <b>welded</b>tail stays intact at one hundred
</pre>

<!-- One char over: prettier dangles the closing `>`, tsv leaves the line intact. -->
<pre>
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <b>welded</b>tail stays intact at one hundred one
</pre>
