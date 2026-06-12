<!-- short multiline value - forces attr break even when content is short -->
<div
	data-attr="a,
  b"
></div>

<!-- self-closing with multiline value - same behavior -->
<input
	data-attr="a,
  b"
/>

<!-- multiple attrs with one multiline value - all attrs break -->
<div
	data-attr1="value1"
	data-attr2="a,
  b"
></div>

<!-- 100 flat boundary: would be exactly 100 if single-line - still breaks due to multiline -->
<div
	data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
  bbbbbbbbbbbbbbbbbbbbbbbbb"
></div>

<!-- 101 flat boundary: would be 101 if single-line - breaks for both reasons -->
<div
	data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
  bbbbbbbbbbbbbbbbbbbbbbbbb"
></div>

<!-- control: single-line attr at 100 chars - does NOT break -->
<div data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"></div>

<!-- control: single-line attr at 101 chars - breaks normally (no multiline) -->
<div
	data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
></div>
