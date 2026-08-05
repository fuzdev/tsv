<script lang="ts">
	// call-body arrow wrapping a source-multiline object: the flat one-line close
	// `}));` is skipped — the last arg gets its own line and the list its own `)`
	obj.aa(x).bb(h, (l) =>
		g({
			v: 1
		})
	);

	// flat inner object stays on one line
	obj.aa(x).bb(h, (l) => g({ v: 1 }));
</script>
