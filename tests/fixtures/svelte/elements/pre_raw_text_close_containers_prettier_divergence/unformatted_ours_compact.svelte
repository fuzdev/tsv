<!--
	The closing tag of a nested `<script>` / `<style>` stays whole at every depth inside `<pre>`:
	under an inline element, a block, a component, and next to a second raw-text element.
-->

<!-- under an inline element -->
<pre><code>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <script>let q = 1;</script> text1</code></pre>

<!-- in a block body -->
<pre>{#if cond}aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <script>let q = 1;</script> text1{/if}</pre>

<!-- in a component -->
<pre><Comp>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <script>let q = 1;</script> text1</Comp></pre>

<!-- two glued raw-text elements: each closing tag stays whole, the second's -->
<pre>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <script>let q = 1;</script><script>let q = 1;</script>text1</pre>

<!-- and the first's, whose split would end the first body at the second closing tag -->
<pre>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <script>let q = 1;</script><script>let q = 1;</script>text1</pre>

<!-- a glued pair of styles, whose first split would end the first body at the second closing tag -->
<pre>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <style>div { color: red; }</style><style>div { color: red; }</style>text1</pre>

<!-- a comment glued after the closing tag -->
<pre>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <script>let q = 1;</script><!-- c --> text1</pre>

<!-- a pre inside other elements -->
<div>
	<div>
		<pre>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <script>let q = 1;</script> text1</pre>
	</div>
</div>
