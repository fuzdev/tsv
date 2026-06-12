<script lang="ts">
	// Sequence with assignment at start
	const a = ((x = y), z) ? b : c;

	// Sequence with assignment in middle
	const b = (a, (x = y), z) ? b : c;

	// Sequence with multiple assignments
	const c = ((x = y), (a = b), z) ? d : e;

	// Sequence only assignments
	const d = ((x = y), (a = b)) ? c : d;

	// Arrow body with sequence
	const e = () => ((a = b), c);

	// Return with sequence
	function fn() {
		return ((a = b), c);
	}

	// Call arg with sequence
	fn(((a = b), c));
</script>
