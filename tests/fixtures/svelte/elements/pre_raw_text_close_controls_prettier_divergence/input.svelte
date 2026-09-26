<!--
	Only a nested `<script>` / `<style>`'s OWN closing tag is held whole inside `<pre>`. Each line
	below is 101 columns wide, and each keeps the layout the rest of the family gives it.
-->

<!-- the parent's closing `>` still dangles -->
<pre>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <b>x<script>let q = 1;</script></b
	> text1</pre>

<!-- a component named Script is no raw-text element: its closing `>` dangles -->
<pre>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <Script>let q = 1;</Script
	> text1 text2</pre>

<!-- nor is an upper-case SCRIPT, whose body Svelte parses as markup -->
<pre>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <SCRIPT>let q = 1;</SCRIPT
	> text1 text2</pre>

<!-- a body already ending on whitespace: the opening `>` breaks, as before -->
<pre>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <script
		>let q = 1; </script> text1 text2</pre>
