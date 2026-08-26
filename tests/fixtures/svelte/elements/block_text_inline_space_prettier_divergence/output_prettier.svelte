<!--
	A text after a block element keeps the space before the inline element that follows it:
	`text3 text4 <span>x</span>` renders `text3 text4 x`. Prettier deletes that space
	(`text3 text4<span>x</span>`, which renders `text3 text4x`) — a content change — whenever the
	text's predecessor is a block element, for a one-word text too, and whether or not the run is the
	fragment's first. A tag or a component after the same text keeps its space under both.
-->

<!-- the space before an inline element after block-then-text, from the same-line authoring -->
<div>
	<div>block1</div>
	text1 text2<span>inline1</span>
</div>

<!-- a run continuing past the element, a one-word text, and a run that is not the fragment's first -->
<div>
	<div>block1</div>
	text1 text2<span>inline1</span> text3
</div>
<div>
	<div>block1</div>
	text1<span>inline1</span>
</div>
<div>
	<span>inline1</span>
	<div>block1</div>
	text1 text2<span>inline2</span>
</div>

<!-- controls: a tag and a component after the same text keep their space under both -->
<div>
	<div>block1</div>
	text1 text2 {expr}
</div>
<div>
	<div>block1</div>
	text1 text2 <Comp />
</div>

<!-- a void and a replaced inline follower: the same deletion, where the render key is blind -->
<div>
	<div>block1</div>
	text1 text2<br /> text3
</div>
<div>
	<div>block1</div>
	text1 text2<input /> text3
</div>

<!-- one run, two answers: only the boundary whose text follows the block loses its space -->
<div>
	<div>block1</div>
	text1<span>inline1</span> text2 <span>inline2</span> text3
</div>

<!-- every block predecessor kind answers alike: a whitespace-sensitive one and a void one -->
<div>
	<pre>block1</pre>
	text1 text2<span>inline1</span>
</div>
<div>
	<hr />
	text1 text2<span>inline1</span>
</div>
