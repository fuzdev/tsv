<!--
	`#x in y` is an expression wherever an expression is read, so it reaches the template
	too — through a spread, and through a comment lead that keeps the brace's interior an
	expression rather than a block marker.
-->
<div {...#x in y}></div>
<div data-attr={/* c */ #x in y}></div>
<textarea>{/* c */ #x in y}</textarea>
