<!--
	A frozen node whose bytes span lines breaks the block element holding it: the content moves to
	its own indented line, and the frozen bytes print as written.
-->

<!-- a frozen element first -->
<p><!-- prettier-ignore --><b   data-attr="value"
>inline1</b>text1</p>

<!-- a frozen element between two texts -->
<div>text2<!-- prettier-ignore --><b   data-attr="value"
>inline2</b>text3</div>

<!-- a frozen expression tag last -->
<p>text4<!-- prettier-ignore -->{a  +
  b}</p>

<!-- a frozen element on one line stays inline -->
<p>text5<!-- prettier-ignore --><b   data-attr="value">inline3</b>text6</p>

<!-- the same element unfrozen collapses -->
<p>text7<b data-attr="value">inline4</b>text8</p>
