{#if aa}
	text1
{:else if bb}
	text2
{/if}

{#each items1 as item1}
	text3
{/each}

{#each items2 as item2 (item2)}
	text4
{/each}

{#await promise1}
	text5
{/await}

{#await promise2 then value}
	text6
{/await}

{#key cc}
	text7
{/key}
