<script lang="ts">
	// 100 visual — hugs (block-body arrow with typed params fits inline)
	const aaaaaaaaaaaaaa = a.fn((b: string, c: ReadonlyArray<string>, d: Record<string, unknown>) => {
		return b;
	});

	// 101 visual — expands call args (arrow moves to its own line)
	const aaaaaaaaaaaaaaa = a.fn(
		(b: string, c: ReadonlyArray<string>, d: Record<string, unknown>) => {
			return b;
		},
	);

	// Short control — stays inline
	const x = a.fn((b: string) => {
		return b;
	});
</script>
