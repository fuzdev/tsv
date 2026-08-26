<!--
	An authored blank line INSIDE a content text is not a run boundary — a run is a partition of
	nodes — so the fill collapses it and the layout is decided as if it were a space, for every
	container kind alike. A blank BESIDE a sibling does bound the run and is preserved (the edge
	control), and authored boundary air still holds the multiline form — a blank spelling of
	that air included, which collapses to the one break it asks for (the air control).
-->

<!-- inline element: the blank collapses, the element stays hugged -->
<div>
	<span><code>a</code> text1 text2

	text3 text4</span>
</div>

<!-- block element: the same -->
<div>
	<p><code>a</code> text1 text2

	text3 text4</p>
</div>

<!-- component: the same -->
<div>
	<Comp><code>a</code> text1 text2

	text3 text4</Comp>
</div>

<!-- control-flow block body: the same -->
<div>
	{#if cond}<code>a</code> text1 text2

	text3 text4{/if}
</div>

<!-- text-only content: the same, with no sibling beside the blank -->
<div>
	<p>text1 text2

	text3 text4</p>
</div>

<!-- control: a blank BESIDE a sibling bounds the run and is preserved, at either edge -->
<div>
	text1 text2

	<span>inline1</span>
</div>
<div>
	<span>inline1</span>

	text1 text2
</div>

<!-- control: authored boundary air holds the multiline form -->
<div>
	<span>
		<code>a</code> text1 text2

		text3 text4
	</span>
</div>
