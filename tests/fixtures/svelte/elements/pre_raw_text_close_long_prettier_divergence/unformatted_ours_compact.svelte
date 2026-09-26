<!--
	Inside `<pre>`, a nested `<script>` / `<style>` is a raw-text element: Svelte ends its body at the
	first literal `</script>` / `</style>`, so its closing tag cannot split. When the line is too
	wide, the break goes before the opening tag's `>` and the closing tag stays whole.
-->

<!-- exactly 100 columns: nothing moves -->
<pre>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <script>let q = 1;</script> text1 text2</pre>

<!-- 101 columns: the opening `>` breaks, the closing tag stays whole -->
<pre>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <script>let q = 1;</script> text1 text2</pre>

<!-- the same for a style -->
<pre>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <style>div { color: red; }</style> text1 text2</pre>

<!-- glued text after the closing tag -->
<pre>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <script>let q = 1;</script>text1 text2</pre>

<!-- the last child -->
<pre>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <script>let q = 1;</script></pre>

<!-- content opening on whitespace: neither delimiter moves, and the line runs over -->
<pre>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <script> let q = 1;</script> text1 text2</pre>

<!-- an empty body: nothing moves, and the line runs over -->
<pre>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <script></script> text1 text2 text3 text4</pre>
