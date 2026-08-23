<script lang="ts">
	let promise;
</script>

{#await promise then a /* c */: A}<div>{a}</div>{/await}
