<script lang="ts">
	interface A {
		a: string;
	}
	interface B {
		b: string;
	}
	let items: unknown;
</script>

<!-- satisfies as the head's outer operator -->
{#each items satisfies A[] as item}
	<div>{item}</div>
{/each}

<!-- satisfies inside an assertion chain -->
{#each items as A satisfies B as item}
	<div>{item}</div>
{/each}

<!-- chain ending on satisfies, with an index -->
{#each items as A[] satisfies B[] as item, i}
	<div>{i}: {item}</div>
{/each}
