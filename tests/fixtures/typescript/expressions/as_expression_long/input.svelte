<script lang="ts">
	// 100 chars total, no break needed
	const a = fn(bbbbbbbbbbbbb) as Record<CCCCCCCCCCCCCCCCCCCCCCCCCCC, DDDDDDDDDDDDDDDDDDDDDDDDDDDDD>;

	// 101 chars, fn(...) as Record< is 100, break type (not call)
	const b = fn(bbbbbbbbbbbbb) as Record<
		CCCCCCCCCCCCCCCCCCCCCCCCCCCC,
		DDDDDDDDDDDDDDDDDDDDDDDDDDDDD
	>;

	// fn(...) as Record< is 100, break type (not call) even with long trailing
	const d = fn(eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee) as Record<
		E,
		F
	>;

	// fn(...) as Record< is 101, break call, trailing 18 chars fits
	const c = fn(
		eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
	) as Record<E, F>;
</script>
