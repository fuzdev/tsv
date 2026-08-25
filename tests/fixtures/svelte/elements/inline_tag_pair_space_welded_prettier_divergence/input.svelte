<!--
	A welded unit beside a spaced separator: the glued boundary is never split (breaking it would
	inject a rendered space), and the spaced one packs per width exactly as it does between two
	lone tags — the unit travels whole and the space beside it is an ordinary fill boundary.
-->

<!-- a glued pair before and after a spaced tag -->
<p>
	{expr1}{expr2} {expr3}
</p>
<p>
	{expr1} {expr2}{expr3}
</p>

<!-- glue that is content: a non-breaking space, and a word glued to both tags -->
<p>
	{expr1}&nbsp;{expr2} {expr3}
</p>
<p>
	{expr1}text1{expr2} {expr3}
</p>
