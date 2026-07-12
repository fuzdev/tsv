<script lang="ts">
	const a = arr[(x = 0)];
	const b = obj?.[(x = 0)];
	arr[(x = 0)] = 1;
	// compound-assignment operator and a non-null assertion on the result
	const c = arr[(x += 1)];
	const d = arr[(x = 0)]!;
</script>
