<!-- a run inside a CSS string: collapsing it changes the font name -->
<div  style:font-family="'Font   Name', sans-serif"  ></div>

<!-- a run between two tokens: safe to collapse in CSS, but spelled like the one above -->
<div  style:transform-origin="top   left"  ></div>

<!-- a run between value parts, which the parts can put inside a string at runtime -->
<div  style:content="'{expr1}   {expr2}'"  ></div>

<!-- the same run with the attribute list broken -->
<div
	style:transform-origin="{expr1}
	{expr2}"

	style:content="'{expr3}   {expr4}'"
></div>

<!-- a newline run at the edge of a text part keeps the authored indent -->
<div
	style:transform="translate({expr1},
      {expr2})"
></div>

<!-- a newline run inside a text part: prettier reflows the author's line break away -->
<div
	style:transform-origin="top
      left"

></div>

<!-- a non-breaking space is content to CSS as well, so the run holding one stays verbatim -->
<div
	style:transform-origin="{expr5}
       {expr6}"

></div>

<!-- a form feed is content to the host, so the run holding one is not respelled either -->
<div
	style:transform-origin="{expr7}
{expr8}"

></div>
