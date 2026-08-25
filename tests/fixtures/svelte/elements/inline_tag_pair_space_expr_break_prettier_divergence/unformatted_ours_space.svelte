<!--
	A spaced tag pair where one expression must break. A tag whose expression breaks expands
	INSIDE its own braces and leaves its adjacencies untouched, so a space-spelled tag after it
	hugs the closing `)}` per width, exactly as an element or a word does after a broken tag;
	the newline spelling of that boundary is held. A tag whose expression cannot fit flat after
	its predecessor travels to a fresh line whole and breaks internally there.
-->

<!-- the first expression breaks: the second tag hugs its closing line -->
<p>
	{fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)} {expr}
</p>

<!-- control: the newline spelling of the same boundary is held -->
<p>
	{fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)}
	{expr}
</p>

<!-- control: the second expression breaks, so it travels whole and breaks on its own line -->
<p>
	{expr} {fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)}
</p>
