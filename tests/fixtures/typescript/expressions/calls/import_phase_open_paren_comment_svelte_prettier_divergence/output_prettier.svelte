<script lang="ts">
	// a `defer`-phased import answers its own `(` like every other call shape — the phase
	// only changes the head, not the gap
	const a = import.defer(
		// c1
		'./a'
	);

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap (the blank ABOVE one stays erased, against the delimiter)
	const b = import.defer(
		// c2

		'./b'
	);
</script>
