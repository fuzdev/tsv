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

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap (the blank ABOVE one stays erased, against the delimiter)
	const c = import.source(
		// c3

		'./c'
	);
</script>
