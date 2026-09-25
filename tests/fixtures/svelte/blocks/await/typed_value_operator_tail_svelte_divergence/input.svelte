<script lang="ts">
	let promise;
</script>

{#await promise then value: T + 1}
	<p>{value}</p>
{/await}
