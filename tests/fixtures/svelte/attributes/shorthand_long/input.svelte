<!-- Single shorthand attribute -->

<!-- 100 chars - stays inline (at boundary) -->
<Comp {aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa} />

<!-- 101 chars - wraps -->
<Comp
	{aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa}
/>

<!-- Multiple shorthand attributes -->

<!-- 100 chars - stays inline (at boundary) -->
<Comp {aaaaaaaaaaaaaaaaaaaaaaaaaaa} {bbbbbbbbbbbbbbbbbbbbbbbbbbb} {ccccccccccccccccccccccccccccc} />

<!-- 101 chars - wraps all attributes -->
<Comp
	{aaaaaaaaaaaaaaaaaaaaaaaaaaaa}
	{bbbbbbbbbbbbbbbbbbbbbbbbbbbb}
	{cccccccccccccccccccccccccccc}
/>
