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
	<span
		><code>a</code>
		<code>b</code> text1</span
	>
</div>
<div>
	<span
		><Comp1 />
		<Comp2 /> text1</span
	>
</div>
<div>
	<span
		>{a}
		{b} text1</span
	>
</div>

<!-- two one-word texts in one run are two labels, not a phrase: the count is per node, so the run
     is no fill and the newline still selects the layout (a sum would have read two words) -->
<div>
	<span
		><code>a</code>
		<code>b</code> text1 <code>c</code> text2</span
	>
</div>

<!-- control: the same newline in a run whose text carries two words — the run is prose and the
     newline is the fill's wrap point -->
<div>
	<span
		>{a}
		{b} text1 text2</span
	>
</div>
