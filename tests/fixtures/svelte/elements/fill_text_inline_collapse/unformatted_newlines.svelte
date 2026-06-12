<!-- Text ending with comma before newline - collapses to fill -->
<div>
	text1 <a href="x">a</a>,
	text2 <a href="y">b</a>.
</div>

<!-- Text ending with period before newline - collapses to fill -->
<div>
	text1 <a href="x">a</a>.
	text2 <a href="y">b</a>.
</div>

<!-- Multiple inline elements with comma text - collapses to fill -->
<nav>
	text1 <a href="x">a</a>,
	text2 <a href="y">b</a>,
	text3 <a href="z">c</a>.
</nav>

<!-- Whitespace-only text between inline elements - preserves multiline (control case) -->
<div>
	text1 <a href="x">a</a>
	text2 <a href="y">b</a>
</div>
