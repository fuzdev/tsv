<!--
	fits: the leading `line` before `ccccc` renders flat, as a space
-->
<li>
	aaaaa ~{xxxxxxxxxx} bbbbb ~{yyyyyyyyyy} ccccc ccccc ccccc
</li>
<!--
	the leading `line` breaks: the tag line sits at exactly 100, so the first `ccccc` cannot
	follow it — the leading `line` becomes the newline itself, NOT a newline plus the space it
	stood for, which would strand a leading space at the head of the continuation line
-->
<li>
	{xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx}
	ccccc ccccc ccccc
</li>
