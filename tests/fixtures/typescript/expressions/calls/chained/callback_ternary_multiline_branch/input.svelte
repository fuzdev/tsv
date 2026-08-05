<script lang="ts">
	// ternary-body callback with a source-multiline branch: the body breaks after
	// => with no parens; the chain stays flat
	obj.aa(x).bb((l) =>
		l
			? {
					v: 1
				}
			: c
	);

	// flat ternary body keeps its parens inline
	obj.aa(x).bb((l) => (l ? v : c));
</script>
