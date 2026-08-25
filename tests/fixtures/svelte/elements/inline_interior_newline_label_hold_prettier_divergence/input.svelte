<!--
	An INTERIOR newline in a hugged inline element selects its layout only when the content is
	not a reflowable fill. A run holding a single word has no fill to reflow into, so its
	whitespace-only newline is the author's structure and the element expands, boundaries and
	all; the same newline in a run that is prose — its text carries two words — is the
	fill's own wrap point, and the element collapses.
-->

<!-- a whitespace-only newline between two elements, two components, or two tags in a one-word
     run: held, and it selects the layout -->
<div>
	<span>
		<code>a</code>
		<code>b</code> text1
	</span>
</div>
<div>
	<span>
		<Comp1 />
		<Comp2 /> text1
	</span>
</div>
<div>
	<span>
		{a}
		{b} text1
	</span>
</div>

<!-- two one-word texts in one run are two labels, not a phrase: the count is per node, so the run
     is no fill and the newline still selects the layout (a sum would have read two words) -->
<div>
	<span>
		<code>a</code>
		<code>b</code> text1 <code>c</code> text2
	</span>
</div>

<!-- the same one-word interior newline spelled at a CONTENT TEXT's edge rather than in a
     whitespace-only node: one class, one answer, whichever node the parser folded the newline
     into. A lone `,` is a word like any other, so its run is a label and the newline is held -->
<div>
	<span>
		<code>a</code> <code>b</code>,
		<code>c</code>
	</span>
</div>
<div>
	<span>
		text1
		<code>a</code>
	</span>
</div>
<div>
	<span>
		<code>a</code>
		text1
	</span>
</div>

<!-- control: the same content-text edge in a run whose text carries two words — prose, so the
     newline is the fill's wrap point and the element collapses -->
<div>
	<span>text1 text2 <code>a</code></span>
</div>

<!-- control: the same newline in a run whose text carries two words — the run is prose and the
     newline is the fill's wrap point -->
<div>
	<span>{a} {b} text1 text2</span>
</div>
