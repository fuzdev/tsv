<script lang="ts">
	let a = $state(0);
	let b = $derived(a * 2);
	let c = $derived.by(() => a * 3);
	let {d, e} = $props();
</script>
