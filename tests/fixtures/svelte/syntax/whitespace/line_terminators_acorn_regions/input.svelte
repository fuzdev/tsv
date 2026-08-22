<script module lang="ts">
	const a = `t1 `;
</script>

<script lang="ts">
	const b = 1;
</script>

<p>{`t2 `}</p>
<p>{b}</p>

<div data-attr={`t3 `}>text1</div>
<div data-attr={b}>text2</div>

<p>text1 text2{b}</p>
<p>text3 text4</p>
<p>{b}</p>

<p>{/* c1 c2 */ b}</p>

{#each [1] as { c = 't4 ' }}
	{@const d = `t5 `}
	{c}{d}
{/each}

{#each [1] as e: ' '}
	{e}
{/each}

{#snippet fn(f = `t6 `)}
	{f}
{/snippet}
