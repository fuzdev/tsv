<!-- 100 chars - fits, stays inline -->
<div>
	<small>{a} {bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb} text</small>
</div>

<!-- 101 chars - overflows; block-style, and the content run packs as one fill -->
<div>
	<small>
		{a}
		{bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb} text
	</small>
</div>

<!-- Element lead, tag glued to its trailing word - same packed layout -->
<div>
	<small>
		<Comp />
		{ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc}text
	</small>
</div>

<!-- The SAME run in an element made multiline by AUTHORED AIR rather than by width - the parity
	assertion: the interior does not depend on why the element expanded -->
<div>
	<small>
		{a}
		{b} text
	</small>
</div>
