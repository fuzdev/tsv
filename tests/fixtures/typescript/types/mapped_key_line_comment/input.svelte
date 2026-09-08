<script lang="ts">
	// Line comment trailing the mapped key constraint (before `]`): the brackets break,
	// `K in T` on its own indented line and the `]` on its own, so the `//` can't
	// swallow it.
	type M = {
		[
			K in T // c
		]: V;
	};
</script>
