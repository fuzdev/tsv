<script lang="ts">
	let count = $state(0);

	$inspect(count);
	$inspect(count).with(console.log);

	$effect(() => {
		$inspect.trace('my effect');
		count;
	});
</script>
