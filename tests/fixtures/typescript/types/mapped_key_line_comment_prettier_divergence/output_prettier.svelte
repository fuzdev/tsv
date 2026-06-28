<script lang="ts">
	// Line comment trailing the mapped key constraint (before `]`): the `]` drops to
	// its own line so the `//` can't swallow it, keeping `K in T` on the `[` line.
	type M = {
		[
			K in T // c
		]: V;
	};
</script>
