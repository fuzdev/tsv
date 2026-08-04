{#if a}
	{@const { b = /* c1 */ 1 } = expr}
	{@const { c = d > e /* c2 */ } = expr}
	{b}{c}
{/if}
