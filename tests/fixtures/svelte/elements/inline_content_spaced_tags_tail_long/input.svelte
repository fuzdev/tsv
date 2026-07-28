<!-- 100 chars - fits, stays inline -->
<div>
	<small>{a} {bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb} text</small>
</div>

<!-- 101 chars - overflows; block-style, and each expression tag takes its own line -->
<div>
	<small>
		{a}
		{bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb} text
	</small>
</div>

<!-- Element lead, tag glued to its trailing word - same one-per-line layout -->
<div>
	<small>
		<Comp />
		{ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc}text
	</small>
</div>
