<!-- An {#await} as the content of an inline element that trailing text follows after a space, at the 100/101 boundary of the block-head line: at 100 the head stays on one line, at 101 it wraps and its } (with any then clause) dangles, the same form the element takes without the trailing text. Prettier never wraps a block head. -->

<!-- head line == 100: the head stays on one line -->
<div>
	<b>
		{#await aaaa || xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx}
			B
		{/await}
	</b> tail
</div>

<!-- head line == 101: the head wraps and its } dangles -->
<div>
	<b>
		{#await aaaa || xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx}
			B
		{/await}
	</b> tail
</div>

<!-- head line == 100 with a then clause -->
<div>
	<b>
		{#await aaaa || xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx then value}
			B
		{/await}
	</b> tail
</div>

<!-- head line == 101 with a then clause: the clause dangles with the } -->
<div>
	<b>
		{#await aaaa || xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx then value}
			B
		{/await}
	</b> tail
</div>

<!-- head line == 100 in an inline element between two text runs -->
<p>
	some words
	<span>
		{#await aaaa || xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx}
			B
		{/await}
	</span> more
</p>

<!-- head line == 101 in an inline element between two text runs -->
<p>
	some words
	<span>
		{#await aaaa || xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx}
			B
		{/await}
	</span> more
</p>
