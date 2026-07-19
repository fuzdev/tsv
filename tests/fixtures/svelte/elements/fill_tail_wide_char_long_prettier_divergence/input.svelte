<!-- Wide (CJK) tail after an expression tag. Each glyph is 2 columns, so the tail is 4 wide,
     not 2 — the fill measures visual width, not chars. This line is exactly 100. -->
<li>
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ~{xxxxxxxxxx} 日本
</li>

<!-- One char more: 101 by visual width, so the wide tail breaks to its own line. A byte- or
     char-based measure would see 99 here and wrongly keep it inline. -->
<li>
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ~{xxxxxxxxxx}
	日本
</li>
