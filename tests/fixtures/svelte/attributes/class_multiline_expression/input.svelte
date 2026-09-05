<!-- expression alone on a continuation line - its indentation is preserved as authored -->
<div
	class="a b
    {expr}
    c"
></div>

<!-- expression ends the value on a continuation line -->
<div
	class="a b
    {expr}"
></div>

<!-- expression followed by text on a continuation line -->
<div
	class="a b
    {expr} c
    d"
></div>

<!-- tab-indented continuation line -->
<div
	class="a b
	{expr}
	c"
></div>

<!-- control: expression on the first line keeps one separator space before it -->
<div
	class="a b {expr}
    c"
></div>
