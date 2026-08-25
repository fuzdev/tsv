<!--
	Space-separated tags pack per width on their content line: the separator between two tags is
	a fill boundary that breaks only before the tag that would cross print width, never with the
	container — one tag per line is never the answer to width.
-->

<!-- a pair whose content line is exactly 100 chars - packs -->
<p>
	{a}
	{bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb}
</p>

<!-- a pair whose content line would be 101 chars - breaks before the second tag -->
<p>
	{a}
	{bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb}
</p>

<!-- three tags in an inline element, the content line exactly 100 chars - all three pack -->
<div>
	<small>
		{a}
		{b}
		{cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc}
	</small>
</div>

<!-- three tags, the content line would be 101 chars - the run breaks before the tag that
	overflows, and the first two stay packed -->
<div>
	<small>
		{a}
		{b}
		{ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc}
	</small>
</div>
