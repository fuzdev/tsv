<script lang="ts">
	// a `source`-phased import answers its own `(` like every other call shape
	const a = import.source(
		// c1
		'./a'
	);

	// a `defer`-phased one likewise — the phase only changes the head, not the gap
	const b = import.defer(
		// c2
		'./b'
	);
</script>
