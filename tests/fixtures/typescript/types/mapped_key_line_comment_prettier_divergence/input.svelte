<script lang="ts">
	// Line comment trailing the mapped key constraint (before `]`): the `]` drops to
	// its own line so the `//` can't swallow it. tsv keeps `K in T` on the `[` line;
	// prettier also breaks after `[`.
	type M = {
		[K in T // c
		]: V;
	};
</script>
