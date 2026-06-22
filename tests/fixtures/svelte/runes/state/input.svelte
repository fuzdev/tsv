<script lang="ts">
	let obj = $state({ n: 0 });
	let snapshot = $state.snapshot(obj);
	let eager = $state.eager(obj.n);
	let raw = $state.raw([1, 2, 3]);
</script>
