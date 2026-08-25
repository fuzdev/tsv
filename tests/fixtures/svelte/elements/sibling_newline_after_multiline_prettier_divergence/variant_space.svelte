<!--
	An authored newline between a unit that RENDERS multiline and the inline SIBLING after it — an
	element, a component, a tag, a void — is preserved, exactly as it is before sibling text
	(tail_newline_after_multiline): beside a tag pile the newline is visible structure and both
	forms are clean, so the authored one is kept. The space spelling stays the per-width hug, so
	the boundary is dual-stable beside a multiline unit (variant_space, where prettier hugs too).
	Beside a unit that renders INLINE the same newline is a spelling the fill reflows (the fitting
	control packs by width), and after a breaking TAG it hugs — the tag's break lands inside its
	own expression, so the tag-pile reading does not arise.
-->
<!-- an inline element after a block-styled element; the element's own text tail reflows -->
<p>
	text1
	<span>
		<b>inline1</b>
	</span> <b>inline2</b> text2
</p>

<!-- a component after a component with content, in an inline container -->
<span>
	text3
	<Comp1>
		<b>inline3</b>
	</Comp1>
	<Comp2 /> text4
</span>

<!-- a tag after an element with a structurally multiline tag content -->
<p>
	text5
	<a href="/a">
		{expr1}
	</a>
	{expr2} text6
</p>

<!-- a void element after an element block-styled by wide text -->
<p>
	text7
	<code>
		alpha bravo charlie delta echo foxtrot golf hotel india juliett kilo lima mike november oscar
	</code> <input /> text8
</p>

<!-- control: after a unit that renders INLINE the newline reflows with the fill -->
<p>
	text9 <span>inline4</span> <b>inline5</b> text10
</p>

<!-- control: after a breaking TAG the sibling hugs -->
<p>
	text11
	{fn(
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	)} <b>inline6</b> text12
</p>
