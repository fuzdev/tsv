<script lang="ts">
	let a;
</script>

<!-- a `<` in one comma sibling, a `>` ahead of a paren in a later one, no list between -->
<Comp prop={{ p: a < b, q: c > (d - e) * 2 }} />
<div data-attr={fn(a < b, c + d > (e, f))}>block</div>
<p>{fn(a < b, c() > (d - e) * 2)}</p>

{#if fn(a < b, c + d > (e, f))}
	<p>text1</p>
{/if}

{#each [a < b, c * d > (e, f)] as item}
	<p>{item}</p>
{/each}

{#if cond}
	{@const obj = { p: a < b, q: c > (d - e) * 2 }}
	<p>{obj.p}</p>
{/if}

<button onclick={() => fn(a < b, (c) => c + d > (e, f))}>text2</button>
