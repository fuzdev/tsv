<!--
	An entity-encoded whitespace character is a SEPARATOR, not prose: `&#9;` decodes to a tab, so
	the node carries no word for a fill to pack even though its raw bytes make it template CONTENT
	to the whitespace scalars. A run of pure siblings with no prose keeps its authored lines, so
	each run below is its own fixed point — exactly like the literal-space control beneath them,
	which the compiler makes the very same document (the entity decodes, then the inter-sibling run
	collapses to one space). Counting the entity as the run's prose made the content a fill and
	flattened it onto one line, deleting a break the author wrote. Its BYTES are never rewritten
	either — the spelling is source the author chose, and respelling it as a tab edits content.
-->
<div>
	<span>   
		<code>a</code>
		<code>b</code>&#9;<code>c</code>
	</span>
</div>

<div>
	<span>   
		<code>a</code>
		<code>b</code>&#x20;<code>c</code>
	</span>
</div>

<div>
	<span>   
		<code>a</code>
		<code>b</code>&Tab;<code>c</code>
	</span>
</div>

<div>
	<span>   
		<code>a</code>
		<code>b</code>	<code>c</code>
	</span>
</div>
