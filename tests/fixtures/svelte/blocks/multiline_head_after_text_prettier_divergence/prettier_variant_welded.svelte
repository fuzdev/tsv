<p>
	text1 {#if cond}
		text2
	{/if}
</p>

<span>
	text3 {#each items as item}
		{item}
	{/each}
</span>

<Comp>
	text4 {#key a}
		text5
	{/key}
</Comp>

<p>
	text6 {#await promise}
		text7
	{:then value}
		{value}
	{/await}
</p>

<p>
	text8 {#if b}text9{/if} text10
</p>

{#if c}
	text11 {#each items2 as item2}
		{item2}
	{/each}
{:else}
	text12 {#if d}
		text13
	{/if}
{/if}
