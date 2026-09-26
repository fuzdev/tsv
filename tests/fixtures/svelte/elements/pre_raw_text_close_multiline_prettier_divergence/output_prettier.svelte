<!--
	A nested `<script>` / `<style>` inside `<pre>` whose body holds a line break and meets both of
	its tags on visible bytes: the line break forces the element open, and at any width the break
	goes before the opening tag's `>` while the closing tag stays whole.
-->

<!-- the only child -->
<pre><script>
		let q = 1;
		let r = 2;
	</script></pre>

<!-- with an attribute, between text -->
<pre>text1 <script data-attr="value">
		let q = 1;
		let r = 2;
	</script> text2</pre>

<!-- a style -->
<pre><style>
		div {
		}
		p {
		}
	</style></pre>
