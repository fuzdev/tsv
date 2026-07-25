<script lang="ts">
	// own-line before a union element: Rule A — the first member freezes
	type A = [
		label:
			// prettier-ignore
			{x:   1} | c1,
		other: b
	];
</script>
