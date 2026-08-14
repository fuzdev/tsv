<!--
	A shorthand attribute's interior is a JS identifier, so it takes the Unicode
	ID_Start / ID_Continue classes rather than Rust's alphanumeric one: U+2118 is
	ID_Start but not alphabetic, and U+200C ZWNJ is ID_Continue but not alphanumeric.
	U+00B2 is the reverse in both arms (see the input_invalid_* files).
-->
<div {℘} {fn‌}></div>
