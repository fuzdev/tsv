<!--
	A nested `<script>` / `<style>` inside `<pre>` keeps its closing tag whole whatever its opening
	tag does: the attribute list wraps as it does on any whitespace-sensitive element, and the `>`
	takes its own line one level in rather than the closing tag splitting.
-->

<!-- attributes wrap -->
<pre><script
		data-attr1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
		data-attr2="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
		>let q = 1;</script></pre>

<!-- attributes wrap, text after -->
<pre><style
		data-attr1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
		data-attr2="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
		>div { color: red; }</style> text1</pre>

<!-- one attribute that fits: only the `>` moves -->
<pre>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <script lang="ts"
		>let q: A = 1;</script> text1 text2</pre>

<!-- a body tsv never formats -->
<pre>aaaaaaaaaaaaaaaaaaaaaaaaa <script type="application/ld+json"
		>{"q": 1}</script> text1 text2</pre>

<!-- a list ending on a `//`: the `>` cannot share its line -->
<pre><script
		data-attr="value" // c
		>let q = 1;</script></pre>

<!-- content opening on whitespace: the `>` hugs the last attribute -->
<pre><script
		data-attr1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
		data-attr2="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"> let q = 1;</script></pre>
