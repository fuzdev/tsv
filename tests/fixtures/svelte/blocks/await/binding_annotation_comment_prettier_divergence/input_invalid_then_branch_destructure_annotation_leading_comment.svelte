<script lang="ts">
	let promise;
</script>

{#await promise}
	loading
{:then { a } /* c */: A}
	<div>{a}</div>
{/await}
