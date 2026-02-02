<script lang="ts">
	// Template literal at 100 chars - no break
	console.log(`xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx`);

	// Template literal at 101 chars - breaks (template literals are "breakable")
	console.log(`xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx`);

	// Regular string at 101 chars - also breaks in Svelte context
	// (the "poorly breakable" behavior only applies in certain pure TS contexts)
	console.log('xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx');
</script>
