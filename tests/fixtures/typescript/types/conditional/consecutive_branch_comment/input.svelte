<script lang="ts">
	// Consecutive line comments after `?` — the first trails `?`, each subsequent
	// one drops to its own line. None merge (`// c1 // c2` would reparse as one).
	type A1 = P extends Q
		? // c1
			// c2
			X
		: Y;

	// The false-branch sibling: consecutive line comments after `:`.
	type A2 = P extends Q
		? X
		: // c1
			// c2
			Y;

	// Three consecutive line comments after `?` — each after the first on its own line.
	type A3 = P extends Q
		? // c1
			// c2
			// c3
			X
		: Y;
</script>
