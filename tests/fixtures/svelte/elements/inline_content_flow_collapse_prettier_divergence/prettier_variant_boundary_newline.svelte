<!--
	An inline element whose content is a run of inline siblings, authored with a newline BETWEEN
	two of them and the content boundaries hugged. The inter-sibling newline flows back onto the
	content line (Svelte 5 collapses the run to one space, so its spelling carries no signal), and
	the hugged boundaries are preserved, so the element converges on the fully inline form. That
	must settle in ONE pass: the flow and the layout it feeds are decided together, not on
	successive passes. Covers an HTML inline element, a table cell (inline-classified).
-->
<div>
	<span>
		<code>a</code> <code>b</code>, <code>c</code> <code>d</code>
	</span>
</div>

<div>
	<td>
		<code>a</code> <code>b</code>, <code>c</code> <code>d</code>
	</td>
</div>
