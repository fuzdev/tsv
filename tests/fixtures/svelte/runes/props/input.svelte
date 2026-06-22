<script lang="ts">
	let {
		name,
		count = $bindable(0),
		id = $props.id()
	}: { name: string; count: number; id: string } = $props();
</script>

<label for={id}>{name}</label>
<input {id} type="number" bind:value={count} />
