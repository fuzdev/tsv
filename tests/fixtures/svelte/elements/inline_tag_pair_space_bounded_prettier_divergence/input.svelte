<!--
	The predecessor never decides a space: a tag after a comment, a <br />, a control-flow
	block or a predecessor that renders multiline takes its per-width wrap exactly as an inline
	element or component does there, so the authored space is kept and the pair packs — where
	prettier breaks its line between two tags with the container. The newline spelling of the
	same boundary is held, so the two spellings are two fixed points. Only a block element
	predecessor keeps the tag off its line; a comment or a control-flow block after the pair
	takes the space as its own per-width wrap (inline_sibling_space_before_bounding); a
	declaration tag takes its own line.
-->

<!-- after a comment: the space spelling hugs, the newline spelling is held; and at the root -->
<p>
	<!-- c --> {expr1} {expr2}
</p>
<p>
	<!-- c -->
	{expr1} {expr2}
</p>
<!-- c --> {expr1} {expr2}

<!-- before a comment, and before a control-flow block that renders inline: the pair keeps its
     space there too — the follower's own per-width wrap, as after text -->
<p>
	{expr1} {expr2} <!-- c -->
</p>
<p>
	{expr1} {expr2} {#if cond}text1{/if}
</p>

<!-- after a <br />, both spellings, and a spaced <br /> between two tags -->
<p>
	text1 text2<br /> {expr1} {expr2}
</p>
<p>
	text1 text2<br />
	{expr1} {expr2}
</p>
<p>
	{expr1} <br /> {expr2}
</p>

<!-- after a block element, whose own break keeps the tag off its line -->
<div>
	<div>block1</div>
	{expr1} {expr2}
</div>

<!-- after a control-flow block that renders multiline, both spellings, and after a declaration
     tag, which takes its own line -->
<div>
	{#if cond}
		<div>block1</div>
	{/if} {expr1} {expr2}
</div>
<div>
	{#if cond}
		<div>block1</div>
	{/if}
	{expr1} {expr2}
</div>
{#if cond}
	{@const x = expr}
	{expr1} {expr2}
{/if}

<!-- after a predecessor that renders multiline: the space spelling hugs its closing tag, as an
     element or component does there, and the newline spelling is held -->
<p>
	<span>
		<div>block1</div>
	</span> {expr1} {expr2}
</p>
<p>
	<span>
		<div>block1</div>
	</span>
	{expr1} {expr2}
</p>
