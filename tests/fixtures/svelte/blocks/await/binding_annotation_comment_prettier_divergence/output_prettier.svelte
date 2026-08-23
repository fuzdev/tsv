<script lang="ts">
	let promise;
</script>

<!-- then-shorthand: an identifier binding's type annotation -->
{#await promise then a: A}<div>{a}</div>{/await}

<!-- :then destructuring binding's type annotation -->
{#await promise}
	loading
{:then { b }: B}
	<div>{b}</div>
{/await}

<!-- :catch identifier binding's type annotation -->
{#await promise}
	loading
{:catch c: C}
	<div>{c}</div>
{/await}
