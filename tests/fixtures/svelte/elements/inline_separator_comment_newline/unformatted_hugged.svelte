<!--
	A comment beside a text run keeps its authored line, and the two sides are INDEPENDENT: a
	newline on one side alone is held on that side alone, so all four authorings below are fixed
	points. Both formatters hold all four, which is why this is not a divergence — it is the
	boundary of tsv's own sibling-newline flow rule, where an inline element or a tag in the
	identical position DOES flow (inline_sibling_newline_flow_prettier_divergence, whose
	prettier_variant_newline_after / _before carry the element twins of the one-sided pair and
	converge on its input).
-->

<!--
	control: newline on BOTH sides — the comment owns its line
-->
<p>text1 text2
<!-- c -->
text3 text4</p>

<!--
	control: newline AFTER only — the following run stays off the comment's line
-->
<p>text1 text2 <!-- c -->
text3 text4</p>

<!--
	control: newline BEFORE only — the comment stays off the preceding run's line
-->
<p>text1 text2
<!-- c --> text3 text4</p>

<!--
	control: NEITHER side — the rule HOLDS what was authored, it does not FORCE a break around a
	comment, so the hugged authoring stays hugged. This is the null control of the pair above,
	varying the same dimension (is there a newline at this boundary?): without it, an
	implementation that always broke both sides of a comment would pass every case above.
-->
<p>text1 text2 <!-- c --> text3 text4</p>
