<script lang="ts">
	// an own-line directive above the signature freezes the whole `[K in ...]: V` clause
	type A = {
		// prettier-ignore
		[K   in   keyof T]:   V;
	};

	// own-line block spelling behaves the same
	type B = {
		/* prettier-ignore */
		[K   in   keyof T]:   V;
	};

	// a multi-line signature keeps its verbatim layout
	type C = {
		// prettier-ignore
		[K in keyof T]: {
			a:   1;
		};
	};
</script>
