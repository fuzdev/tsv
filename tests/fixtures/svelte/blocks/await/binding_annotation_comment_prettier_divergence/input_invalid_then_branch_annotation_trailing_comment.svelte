<script lang="ts">
	let promise;
</script>

{#await promise}
	loading
{:then a: A /* c */}
	<div>{a}</div>
{/await}
