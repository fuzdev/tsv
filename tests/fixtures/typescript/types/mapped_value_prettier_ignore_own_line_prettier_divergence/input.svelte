<script lang="ts">
	// an own-line directive before the value type stays own-line and freezes it
	type A = {
		[K in keyof T]:
			// prettier-ignore
			{x:   1};
	};

	// union value: Rule A — the first member freezes
	type B = {
		[K in keyof T]:
			// prettier-ignore
			{x:   1} | c1;
	};
</script>
