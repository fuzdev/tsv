<!--
	The sibling-newline flow rule counts prose per RUN, and a run ends at whatever owns its own
	line: a comment, a block element, a control-flow block, a `<br />`, and an authored blank
	line — whether that blank stands in a whitespace-only node between two siblings or at the
	edge of a content text. Every case puts a two-word run on one side of the boundary and a
	one-word run on the other: the prose flows right up to the boundary (bounding is not
	sterilizing), and the label beyond it holds, where the same nodes with no boundary between
	them are one run and the label flows with the prose. Prettier holds every authored newline
	here; the flowing halves are the divergence.
-->

<!-- a comment bounds the run: the prose run before it flows up to it, the one-word run after it holds -->
<p>
	text1 text2 <span>inline1</span>
	<!-- c -->
	text3
	<span>inline2</span>
</p>

<!-- a blank line in a whitespace-only node bounds the run -->
<p>
	text1 text2 <span>inline1</span>

	<span>inline2</span>
	text3
</p>

<!-- a blank line at a content text's LEADING edge bounds the run before it -->
<p>
	text1 text2 <span>inline1</span>

	text3
	<span>inline2</span>
</p>

<!-- a blank line at a content text's TRAILING edge bounds the run after it -->
<p>
	<span>inline1</span> text1 text2

	<span>inline2</span>
	text3
</p>

<!-- a block element, a control-flow block and a `<br />` bound the run -->
<div>
	text1 text2 <span>inline1</span>
	<div>block1</div>
	text3
	<span>inline2</span>
</div>
<p>
	text1 text2 <span>inline1</span>
	{#if cond}inline2{/if}
	text3
	<span>inline3</span>
</p>
<p>
	text1 text2 <span>inline1</span>
	<br />
	text3
	<span>inline2</span>
</p>

<!-- control: the same nodes with no boundary are one run, and the label flows with the prose -->
<p>
	text1 text2 <span>inline1</span> text3 <span>inline2</span>
</p>
