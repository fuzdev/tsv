<!-- Short - attrs fit on one line -->
<textarea class="class1"></textarea>

<!-- Long - attrs wrap, > on its own line (inline whitespace-sensitive element) -->
<textarea
	class="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	id="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
	data-attr="ccccccccccccccccccccccccccc"
></textarea>

<!-- With content - whitespace-sensitive, hug behavior -->
<textarea class="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" id="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
	>content</textarea
>

<!-- With expression value - wraps attrs, > on its own line -->
<textarea
	bind:value={expr}
	class="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	placeholder={value || 'text'}
></textarea>
