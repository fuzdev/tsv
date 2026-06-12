<!-- Expression on its own line - preserve multiline -->
<a href="x">
	{expr}
</a>

<!-- Multiple expressions on their own lines - preserve multiline -->
<span data-attr="value">
	{expr1}
	{expr2}
</span>

<!-- Component on its own line - preserve multiline -->
<a href="x">
	<Comp />
</a>

<!-- Nested element on its own line - preserve multiline -->
<span>
	<strong>text</strong>
</span>

<!-- Expression with adjacent text - preserve multiline -->
<a href="x">
	{expr}text
</a>

<!-- Text before expression - preserve multiline -->
<span>
	text{expr}
</span>

<!-- Text-only on separate line - COLLAPSES to single line (different behavior) -->
<a href="x"> text </a>
