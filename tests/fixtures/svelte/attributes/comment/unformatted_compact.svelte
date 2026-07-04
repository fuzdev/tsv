<!-- Line comment between attributes -->
<div
data-attr1="value1"
// line comment
data-attr2="value2"
>
text1
</div>

<!-- Block comment between attributes -->
<div
data-attr1="value1"
/* block comment */
data-attr2="value2"
>
text2
</div>

<!-- Comment before first attribute -->
<div
// before first
data-attr="value"
>
text3
</div>

<!-- Multiple consecutive comments -->
<div
data-attr1="value1"
// first
// second
data-attr2="value2"
>
text4
</div>

<!-- Trailing comment after last attribute -->
<div
data-attr="value"
// trailing
>
text5
</div>

<!-- Multiline block comment (content preserved verbatim, not dedented) -->
<div
data-attr1="value1"
/* multiline block
		line2
		line3
	*/
data-attr2="value2"
>
text6
</div>

<!-- Inline block comment -->
<div data-attr1="value1"/* inline */data-attr2="value2">text7</div>
