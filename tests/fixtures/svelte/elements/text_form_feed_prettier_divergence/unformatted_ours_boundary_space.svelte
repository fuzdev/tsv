<!--
	A form feed (U+000C) is rendered CONTENT, not collapsible whitespace: CSS white-space
	processing reaches only U+0020, U+0009 and segment breaks, and Svelte's compiler agrees
	(`clean_nodes` keeps it verbatim). So it survives every position a space would collapse in,
	and it never turns into one.
-->

<!-- At a content boundary, where a space is trimmed as render-free -->
<span> <code>a</code> </span>

<!-- As the separator between two siblings, where a space collapses to one space -->
<span><code>a</code><code>b</code></span>

<!-- As an element's only content, where a whitespace-only element collapses to <span></span> -->
<span></span>

<!-- Inside prose, where a run of spaces collapses to one -->
<span>text1text2</span>

<!-- The render-free boundary run still trims — it stops AT the form feed, which is content -->
<span> <code>a</code> </span>

<!-- Root-level text, where the surrounding fragment edges trim -->
text1text2
