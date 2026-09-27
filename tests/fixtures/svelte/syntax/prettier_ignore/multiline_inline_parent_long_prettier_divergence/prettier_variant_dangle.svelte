<!--
	A frozen node whose bytes span lines breaks the inline element holding it, which lays out
	block-style: the content moves to its own indented line, and the frozen bytes print as written.
-->

<!-- the text after the frozen element ends its line at exactly 100: it stays -->
<label
	><!-- prettier-ignore --><b   data-attr="value"
>inline1</b>text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 text13 abcdef</label
>

<!-- the text after the frozen element would end its line at 101: the last word wraps -->
<label
	><!-- prettier-ignore --><b   data-attr="value"
>inline2</b>text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 text13
	abcdefg</label
>

<!-- a frozen element between two texts -->
<span
	>text14<!-- prettier-ignore --><b   data-attr="value"
>inline3</b>text15</span
>

<!-- a frozen expression tag last, in a component -->
<Comp
	>text16<!-- prettier-ignore -->{a  +
  b}</Comp
>

<!-- a frozen element in an inline element inside another: both lay out block-style -->
<span
	><em
		><!-- prettier-ignore --><b   data-attr="value"
>inline4</b>text17</em
	></span
>

<!-- a frozen element on one line stays inline -->
<span>text18<!-- prettier-ignore --><b   data-attr="value">inline5</b>text19</span>

<!-- the same element unfrozen collapses -->
<span>text20<b data-attr="value">inline6</b>text21</span>
