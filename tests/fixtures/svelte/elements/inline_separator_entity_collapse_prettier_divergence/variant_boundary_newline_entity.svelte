<!--
	An entity-encoded whitespace separator standing between two non-text siblings is
	interchangeable with the space it decodes to: the compiler collapses the inter-sibling run to
	one whitespace either way, so `&#9;` and a literal space are one document and must reach the
	one fully inline form, with the render-free content boundaries trimmed. Covers the three
	non-text sibling kinds — an expression-tag pair, an element pair and a component pair.
-->
<div>
	<span>
		{a}&#9;{b}
	</span>
</div>

<div>
	<span>
		<code>a</code>&#9;<code>b</code>
	</span>
</div>

<div>
	<span>
		<Comp prop="value" />&#9;<Comp prop="value" />
	</span>
</div>
