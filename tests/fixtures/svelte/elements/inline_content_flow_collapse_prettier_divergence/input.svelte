<!--
	An inline element whose content is a run of inline siblings, authored with a newline BETWEEN
	two of them and the content boundaries hugged. The inter-sibling newline flows back onto the
	content line (Svelte 5 collapses the run to one space, so its spelling carries no signal), and
	the hugged boundaries are preserved, so the element converges on the fully inline form. That
	must settle in ONE pass: the flow and the layout it feeds are decided together, not on
	successive passes. Covers an HTML inline element, a table cell (inline-classified), and the
	rest of the fill class: a component pair and an expression-tag pair whose ONLY separator
	carries words, a lone element with prose leading and trailing, and a pair whose newline sits
	in a WHITESPACE-ONLY separator node rather than in a content text. A run is a fill whenever
	it holds prose AND a whitespace seam to reflow at, so neither the separator's shape nor the
	sibling count decides the layout — only the absence of a seam does (glued `{expr}text` keeps
	its authored lines, pinned by `inline_multiline_nontext`).
-->
<div>
	<span><code>a</code> <code>b</code>, <code>c</code> <code>d</code></span>
</div>

<div>
	<td><code>a</code> <code>b</code>, <code>c</code> <code>d</code></td>
</div>

<div>
	<span><Comp prop="value" /> text1 <Comp prop="value" /></span>
</div>

<div>
	<span>text1 <code>a</code></span>
</div>

<div>
	<span>{a} text1 {b}</span>
</div>

<div>
	<span><code>a</code> text1</span>
</div>

<div>
	<span><code>a</code> <code>b</code> text1</span>
</div>
