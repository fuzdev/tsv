<!--
	The follower never turns a space into a newline either: a space before a COMMENT, a
	`{@debug}` or a control-flow block that renders inline is that follower's own per-width wrap
	after any sibling, exactly as it already is after text — a comment's line is authorship, and
	it is the NEWLINE spelling that holds it. Prettier keeps the space after text and breaks it
	with the container after an element, a component or a tag.
-->

<!-- a comment after an inline element, a component and every tag kind: the space is kept -->
<div>
	<span>inline1</span>   <!-- c -->
	<Comp />   <!-- c -->
	{expr}   <!-- c -->
	{@render fn()}   <!-- c -->
	{@html a}   <!-- c -->
</div>

<!-- and after a bounding predecessor: an inline-rendering control-flow block, a <br />, a
     control-flow block that renders multiline and an element that renders multiline -->
<div>
	{#if cond}text1{/if}   <!-- c -->
	text1 text2<br />   <!-- c -->
</div>
<div>
	{#if cond}
		<div>block1</div>
	{/if}   <!-- c -->
</div>
<div>
	<span>
		<div>block1</div>
	</span>   <!-- c -->
</div>

<!-- the same at the root -->
<span>inline1</span>   <!-- c -->
{expr}   <!-- c -->

<!-- a `{@debug}` is the same follower kind, after an element and after a tag at the root. Both
     are INTERIOR: a `{@debug}` is hoisted, so at a fragment EDGE the run beside it is render-free
     and trims instead (blocks/hoisted_boundary_sibling_kinds) -->
<div>
	<span>inline1</span>   {@debug a}   text1
</div>
{expr}   {@debug a}

<!-- an inline-rendering control-flow block after an inline element, a tag, a component and a
     comment — every block kind -->
<div>
	<span>inline1</span>   {#if cond}text1{/if}
	{expr}   {#if cond}text1{/if}
	<Comp />   {#each items as item}{item}{/each}
	<!-- c -->   {#if cond}text1{/if}
	<span>inline1</span>   {#await promise}text1{/await}
	<span>inline1</span>   {#key value}text1{/key}
</div>

<!-- a comment spaced on both sides, between two tags, two components and two elements, at the
     root: both spaces are kept -->
{expr1}   <!-- c -->   {expr2}
<Comp1 />   <!-- c -->   <Comp2 />
<span>inline1</span>   <!-- c -->   <span>inline2</span>

<!-- two comments: the second keeps its space after the first -->
<div>
	<!-- c1 -->   <!-- c2 -->
</div>

<!-- parity controls: after TEXT both formatters keep the space already -->
<div>
	text1 text2   <!-- c -->
	text1 text2   {#if cond}text1{/if}
</div>

<!-- controls: a control-flow block that renders multiline drops to a fresh line whole, a
     block element predecessor keeps the comment off its line, and a <br /> after a comment
     takes its wrap — under both formatters -->
<div>
	<span>inline1</span>
	{#if cond}
		<div>block1</div>
	{/if}
</div>
<div>
	<div>block1</div>
	<!-- c -->
</div>
<div>
	<!-- c -->   <br />
</div>

<!-- the width boundary: the comment and the inline block hug at exactly 100 chars and drop at
     101 -->
<div>
	<span>inline1</span>   <!-- cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc -->
</div>
<div>
	<span>inline1</span>
	<!-- ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc -->
</div>
<div>
	<span>inline1</span>   {#if cond}cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc{/if}
</div>
<div>
	<span>inline1</span>
	{#if cond}ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc{/if}
</div>
