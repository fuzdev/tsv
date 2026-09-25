<script lang="ts">
	let promise;
</script>

{#await promise}
	<p>text</p>
{:then value: T satisfies U}
	<p>{value}</p>
{/await}
