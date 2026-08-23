<script lang="ts">
	let promise;
</script>

{#await promise then a: A /* c */}<div>{a}</div>{/await}
