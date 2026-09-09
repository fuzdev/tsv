<script lang="ts">
	// The operand separators stay break points: a run that fits at 100 columns stays
	// on one line, and the deferred comment is not measured with it.
	const x1 = (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) as A; // c1

	// One column over, the run breaks one operand per line.
	const x2 = (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
	bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) as A; // c2
</script>
