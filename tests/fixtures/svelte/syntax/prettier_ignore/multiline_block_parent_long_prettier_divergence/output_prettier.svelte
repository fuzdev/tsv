<!--
	The line after a frozen node's bytes fills from the column where they end, and wraps at the
	print width like any other line.
-->

<!-- a space after the frozen element: the line ends at exactly 100 and stays -->
<div>
	text1<!-- prettier-ignore --><b   data-attr="value"
>inline1</b> text2 word00 word01 word02 word03 word04 word05 word06 word07 word08 word09 zzzzzzz end
</div>

<!-- a space after the frozen element: the line would end at 101, so the last word wraps -->
<div>
	text1<!-- prettier-ignore --><b   data-attr="value"
>inline1</b> text2 word00 word01 word02 word03 word04 word05 word06 word07 word08 word09 zzzzzzzz end
</div>

<!-- glued to an unindented last frozen line: the line would end at 101, so the last word wraps -->
<div>
	text1<!-- prettier-ignore --><b   data-attr="value"
>inline1</b>text2 word00 word01 word02 word03 word04 word05 word06 word07 word08 word09 zzzzzzzzz
	end
</div>

<!-- an indented last frozen line, glued: the line ends at exactly 100 and stays -->
<div>
	text1<!-- prettier-ignore --><b   data-attr="value"
	>inline1</b>text2 word00 word01 word02 word03 word04 word05 word06 word07 word08 word09 zzzzzz end
</div>

<!-- an indented last frozen line, glued: the line would end at 101, so the last word wraps -->
<div>
	text1<!-- prettier-ignore --><b   data-attr="value"
	>inline1</b>text2 word00 word01 word02 word03 word04 word05 word06 word07 word08 word09 zzzzzzz end
</div>
