<!--
	A run holding a single word is a LABEL, not prose. Flowing means reflowing a run per width,
	and a fill needs a phrase to reflow into: the run's prose is the most words any ONE of its
	text nodes carries, so one word beside an element, a void element, a component or a tag is a
	caption beside its icon, a field beside its unit, a label beside its value — and the
	sibling-newline flow rule holds the authored newlines beside it exactly as it holds a
	prose-free run's. The count is asked of the RUN (every content text between two run-bounding
	siblings), never of the node at the boundary alone: a one-word node that ends a real sentence
	flows with it.
	Prettier holds every label shape too, so those agree; the flowing controls are where tsv
	converges the authorings and prettier keeps each.
-->

<!-- a word before a void element, and a word after one -->
<label>
	text1 <input type="range" />
</label>
<label>
	<input type="checkbox" /> text1
</label>

<!-- a word before an inline element, and a word after one -->
<p>
	text1 <span>inline1</span>
</p>
<p>
	<span>inline1</span> text1
</p>

<!-- a word between two components -->
<p>
	<Comp1 /> text1 <Comp2 />
</p>

<!-- a word beside a render tag, and beside an html tag -->
<p>
	{@render icon()} text1
</p>
<p>
	{@html svg} text1
</p>

<!-- a word before a tag, and a word after one: a tag renders as a value, and a label beside it
     holds like any other -->
<p>
	text1 {expr}
</p>
<p>
	{expr} text1
</p>

<!-- an element, a word and a tag; a word ending a run of tags -->
<p>
	<Comp /> text1 {expr}
</p>
<p>
	{expr1}
	{expr2}
	{expr3} text1
</p>

<!-- a word after a tag: the separator between the two tags holds with it -->
<p>
	text1 {expr1}
	{expr2}
</p>

<!-- a one-word tail after an element whose own content is prose: that content is the element's
     own run, so the tail's run holds one word -->
<p>
	<a href="/path">inline1 inline2</a> text1 {expr}
</p>

<!-- control: the cliff — a node carrying two words is prose, and the run flows -->
<p>
	<Comp /> text1 text2 {expr}
</p>

<!-- control: run-level, not boundary-local — a one-word node ending a sentence flows with it -->
<p>
	text1 text2 text3 <span>inline1</span> text4
</p>

<!-- control: run-level reaches every boundary in the run, however far it sits from the phrase -->
<p>
	text1 text2 <span>inline1</span> text3 <span>inline2</span> text4
</p>

<!-- control: the cliff in a list — one two-word caption is prose, and it packs the one-word
     captions in its run with it: the cost of any count at the cliff, stated so it is recorded -->
<div>
	<Comp1 /> text1 <Comp2 /> text2 text3
</div>

<!-- two one-word captions in one run, and one-word fragments between tags: the count is the most
     words any ONE node carries, never a sum over the run, so these are labels and hold -->
<label>
	text1 <input type="range" /> text2 <input type="range" />
</label>
<p>
	text1 {expr1} text2 {expr2}
</p>

<!-- a list of one-word captions holds however many it holds: the count is per node -->
<div>
	<Comp1 /> text1 <Comp2 /> text2 <Comp3 /> text3
</div>

<!-- the count's own cost, stated: a sentence spelled entirely as one-word fragments between
     siblings is three labels, and holds — real prose has a two-word node somewhere in its run -->
<p>
	text1 <span>inline1</span> text2
</p>
