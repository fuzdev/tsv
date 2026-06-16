<!-- Short - attrs fit on one line -->
<textarea class="class1"></textarea>

<!-- Long - empty, attrs wrap; closing > hugs the last attr (whitespace-sensitive) -->
<textarea
	class="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	id="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
	data-attr="ccccccccccccccccccccccccccc"></textarea>

<!-- With content - whitespace-sensitive, hug behavior -->
<textarea class="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" id="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
	>content</textarea
>

<!-- With expression value - empty, attrs wrap; > hugs the last attr -->
<textarea
	bind:value={expr}
	class="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	placeholder={value || 'text'}></textarea>
