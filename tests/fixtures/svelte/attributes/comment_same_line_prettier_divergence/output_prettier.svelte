<!-- Line comment after the tag name stays on the tag-name line (prettier moves it to its own line) -->
<div
	// foo
	data-attr="value"
>
	text
</div>

<!-- Between-attribute line comment stays trailing the previous attribute (prettier moves it to its own line) -->
<div
	data-attr1="value1"
	// mid
	data-attr2="value2"
>
	text
</div>

<!-- Combined: after-tag-name comment + trailing comment on a shorthand attribute -->
<div
	// foo
	{value} // bar
>
	text
</div>
