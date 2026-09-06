<!-- Short - attrs fit on one line -->
<textarea class="class1"></textarea>

<!-- Long - empty, attrs wrap; closing > hugs the last attr (whitespace-sensitive) -->
<textarea
	class="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	id="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
	data-attr="ccccccccccccccccccccccccccc"></textarea>

<!-- With content, head exactly 100 - attrs stay flat, `>` takes the next line one level in -->
<textarea data-attr1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" data-attr2="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
	>text</textarea
>

<!-- One char more - the attrs wrap one per line and the `>` hugs the last -->
<textarea
	data-attr1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	data-attr2="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb">text</textarea
>

<!-- With expression value - empty, attrs wrap; > hugs the last attr -->
<textarea
	bind:value={expr}
	class="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	placeholder={value || 'text'}></textarea>
