<!-- Single spread attribute -->

<!-- 100 chars - stays inline (at boundary) -->
<Comp
	{...aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa}
/>

<!-- 101 chars - wraps -->
<Comp   {...aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa}   />

<!-- Multiple spread attributes -->

<!-- 100 chars - stays inline (at boundary) -->
<Comp
	{...aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa}
	{...bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb}
/>

<!-- 101 chars - wraps all attributes -->
<Comp   {...aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa}   {...bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb}   />
