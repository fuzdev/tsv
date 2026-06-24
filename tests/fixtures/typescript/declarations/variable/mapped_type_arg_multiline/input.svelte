<script lang="ts">
	// An authored-multiline mapped type-arg force-breaks, so the call stays on the `=` line.
	const a = fn<{
		[K in keyof T]: V;
	}>();

	// A nested multiline value force-breaks the mapped, so the call stays on the `=` line.
	const b = fn<{
		[K in keyof T]: {
			x: number;
			y: string;
		};
	}>();
</script>
