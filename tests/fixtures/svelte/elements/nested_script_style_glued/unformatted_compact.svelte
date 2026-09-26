<!--
	A nested `<script>` / `<style>` is an ordinary element that renders no box, so the content on
	either side of it meets directly: glued to content on both sides it keeps both glued
	boundaries, and its body breaks onto its own indented lines between the two intact tags.
-->


<!-- text on both sides -->
<div>text1<script>let a = 1;</script>text2</div>

<!-- expression tags on both sides, a multi-line body -->
<p>{expr1}<style>div { color: red; } span { color: blue; }</style>{expr2}</p>

<!-- inline elements on both sides -->
<div><b>inline1</b><script>let b = 2;</script><i>inline2</i></div>

<!-- a comment inside the glued run -->
<div>text1<!-- c --><script>let c = 3;</script>text2</div>

<!-- two raw-text elements glued together -->
<div>text1<script>let d = 4;</script><style>div { color: red; }</style>text2</div>

<!-- an empty script beside a non-empty style -->
<div>text1<script></script><style>div { color: red; }</style>text2</div>

<!-- a whitespace-only body -->
<div>text1<script>
</script>text2</div>

<!-- a frozen body -->
<div>text1<script type="application/ld+json">
		{ "a": 1 }
	</script>text2</div>

<!-- a frozen body in a language tsv does not format -->
<div>text1<script lang="coffee">l = 12</script>text2</div>

<!-- other raw-text and template elements glued on both sides stay inline -->
<div>text1<textarea>text2</textarea>text3</div>

<div>text1<noscript>text2</noscript>text3</div>

<div>text1<template>text2</template>text3</div>
