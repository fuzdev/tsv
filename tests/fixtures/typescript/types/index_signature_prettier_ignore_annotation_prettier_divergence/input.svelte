<script lang="ts">
	// an own-line directive between an index signature's `]` and its value `:` stays
	// own-line and freezes the whole `: type` annotation — interface and type literal
	interface A {
		[k: string]
			// prettier-ignore
			: {x:   1};
	}

	type B = {
		[k: string]
			// prettier-ignore
			: {y:   2};
	};
</script>
