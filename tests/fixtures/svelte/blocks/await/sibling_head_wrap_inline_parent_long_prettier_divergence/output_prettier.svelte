<!-- An {#await} after a breakable sibling (an expression tag, a component, or an inline element and a space) inside an INLINE parent, at the 100/101 boundary of the block-head line: at 100 the head stays on one line, at 101 it wraps and its } dangles, exactly as under a block parent. Prettier never wraps a block head. -->

<!-- head line == 100: the head stays on one line -->
<Comp>
	{expr}{#await fn(argument1, xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx)}
		text
	{/await}
</Comp>

<!-- head line == 101: the head wraps -->
<Comp>
	{expr}{#await fn(argument1, xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx)}
		text
	{/await}
</Comp>

<!-- head line == 100 after a component sibling, inside an inline element -->
<span>
	<Comp />{#await fn(argument1, xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx)}
		text
	{/await}
</span>

<!-- head line == 101 after a component sibling, inside an inline element -->
<span>
	<Comp
	/>{#await fn(argument1, xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx)}
		text
	{/await}
</span>

<!-- head line == 100 on its own line, after a SPACED inline-element sibling -->
<Comp>
	<span>text</span>
	{#await fn(argument1, xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx)}
		text
	{/await}
</Comp>

<!-- head line == 101 on its own line, after a SPACED inline-element sibling -->
<Comp>
	<span>text</span>
	{#await fn(argument1, xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx)}
		text
	{/await}
</Comp>
