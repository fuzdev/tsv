<!--
	A property value's broken binary/logical RHS sits FLUSH with its left operand (the
	assignment-like layout), identical to the same construct in <script>. Only the
	expression-ROOT binary takes the continuation indent (the final case). A value that
	fits at 100 stays on the key's line.
-->
<div
	class={[
		{
			fits: aaaaaaaaaaaaaaaaaaaaaaaaaaa === fnnnnnnnnnn('/path/' + b.name + 'xxxxxxxxxxxxxxxxxxxxx')
		},
		{
			selected:
				aaaaaaaaaaaaaaaaaaaaaaaaaaa ===
				fnnnnnnnnnn('/path/' + b.name + '/references/' + c.slug + 'xxxxxxxxxxxxx')
		},
		{
			active:
				aaaaaaaaaaaaaaaaaaaaaaaaaaa &&
				fnnnnnnnnnn('/path/' + b.name + '/references/' + c.slug + 'xxxxxxxxxxxxx')
		}
	]}
>
	x
</div>

<!-- Root control: the expression-root binary takes the continuation indent in both. -->
<div
	class={aaaaaaaaaaaaaaaaaaaaaaaaaaa ===
		fnnnnnnnnnn('/path/' + b.name + '/references/' + c.slug + 'xxxxxxxxxxxxxxxxxxxxxxxxxxxx')}
>
	x
</div>
