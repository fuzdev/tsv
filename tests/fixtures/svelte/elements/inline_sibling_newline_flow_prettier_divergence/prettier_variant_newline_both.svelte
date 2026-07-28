<!--
	An inline sibling isolated by authored newlines flows back onto the content line. Svelte 5
	collapses inter-sibling whitespace to one space, so a newline and a space between two siblings
	render identically and the newline carries no signal the fill must honor. What an authored
	newline does still decide is the element's own layout: a newline anywhere in the content keeps
	the element multiline (want air, author multiline), so these converge on the multiline form
	rather than collapsing onto one line. Prettier holds a distinct stable form for each authoring.
	Covers an inline element, a component, an expression tag and a render tag. The controls pin
	what does not flow: a control-flow block, whose width is not fixed (packing a run of them
	overflows the line, and the overflow is paid by expanding their bodies); the fully hugged
	authoring, which stays hugged; a comment, which keeps its authored line; a blank line, which
	still breaks; and a block sibling, which still takes its own line.
-->
<p>
	text1
	<span>inline1</span>
	text2 text3
</p>
<p>
	text1
	<Comp prop="value" />
	text2 text3
</p>
<p>
	text1
	{expr}
	text2 text3
</p>
<p>
	text1
	{@render fn()}
	text2 text3
</p>

<!-- control: a control-flow block keeps its authored line — packing a run of them onto one
     line overflows it, and the overflow is paid by expanding their bodies -->
<p>
	text1
	{#if cond}inline1{/if}
	text2 text3
</p>

<!-- control: the fully hugged authoring stays hugged -->
<p>text1 <span>inline1</span> text2 text3</p>

<!-- control: a comment keeps its authored line -->
<p>
	text1
	<!-- c -->
	text2
</p>

<!-- control: a blank line still breaks -->
<p>
	text1

	<span>inline1</span>

	text2
</p>

<!-- control: a block sibling takes its own line; the inline run before it still flows -->
<div>
	text1 <span>inline1</span> text2
	<div>block1</div>
	text3 text4
</div>
