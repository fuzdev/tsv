<!--
	What the sibling-newline flow rule counts as a WORD: a content text's collapsible-whitespace-
	separated words, split over the SOURCE BYTES exactly as the fill splits them into items. An
	NBSP-joined pair is one word (it is one fill item), and so is an entity-encoded space
	(`&#32;` — the fill never breaks inside it); punctuation alone is a word, a hyphenated pair is
	one, and a word glued to a tag is still a word. A run holding one word is a label and holds;
	two words is the cliff, at a void element as at any sibling. Prettier holds every line here;
	the flowing controls are the divergence.
-->

<!-- an NBSP-joined pair is one word -->
<p>
	<Comp />
	text1&nbsp;text2
</p>

<!-- an entity-encoded space is one word too: the fill splits the source bytes and carries
     `text1&#32;text2` as one unbreakable item (prettier does the same), so there is no seam to
     reflow at, and the pair holds exactly as the NBSP pair does -->
<p>
	<Comp />
	text1&#32;text2
</p>

<!-- punctuation alone is a word; a hyphenated pair is one; a word glued to a tag is one -->
<p>
	<a href="/path">inline1</a>
	.
</p>
<p>
	<Comp />
	text1-text2
</p>
<p>
	<Comp />
	{expr}text1
</p>

<!-- a word between two tags -->
<p>
	{expr1}
	text1
	{expr2}
</p>

<!-- punctuation alone between two tags is one word: a connector, and holds -->
<p>
	{expr1}
	·
	{expr2}
</p>

<!-- control: punctuation and a word are two words, and flow -->
<p>
	<a href="/path">inline1</a>
	, and
	<b>inline2</b>
</p>

<!-- control: the cliff at a void element — two words beside a checkbox are prose, and flow -->
<label>
	<input type="checkbox" />
	text1 text2
</label>

<!-- control: a newline inside a text node separates words as a space does, so this node is
     two words, and flows -->
<p>
	<Comp />
	text1 text2
</p>
