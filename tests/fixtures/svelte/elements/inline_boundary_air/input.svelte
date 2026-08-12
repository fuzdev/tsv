<!--
	Authored boundary air survives in an INLINE-classified container exactly as it does in a
	component and a block. The container's classification is not part of the question: the
	author's newlines on both boundaries are the only way to ask for air around content that
	holds a whitespace seam, so every kind honors them. Both formatters agree.

	Asked of ONE boundary the kinds answer differently, and that arity is pinned next door
	(boundary_air_one_sided). The air is also independent of what the interior then does with
	itself: the last case keeps its siblings one per line, because a prose-free run has no fill
	to reflow into — the boundary rule and the interior rule are separate questions.
-->

<!-- inline element -->
<div>
	<span>
		text1 <b>x</b> text2
	</span>
</div>

<!-- component: the parity assertion, same run -->
<div>
	<Comp>
		text1 <b>x</b> text2
	</Comp>
</div>

<!-- block: the same again -->
<div>
	<p>
		text1 <b>x</b> text2
	</p>
</div>

<!-- the other inline-classified kinds take the same answer -->
<div>
	<td>
		text1 {a} text2
	</td>
</div>

<div>
	<label>
		text1 {a} text2
	</label>
</div>

<!-- control: no air authored - the content stays inline -->
<div>
	<span>text1 <b>x</b> text2</span>
</div>

<!-- the air is preserved, but the INTERIOR still follows its own rule: a prose-free run has no
	fill to reflow into, so its siblings keep one line each -->
<div>
	<span>
		{a}
		{b}
	</span>
</div>
