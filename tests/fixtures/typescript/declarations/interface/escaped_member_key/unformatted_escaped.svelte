<script lang="ts">
	// Interface member keys decode from escapes (also covers type-literal keys: same parser path)
	interface I {
		\u0069n: string;
		\u0062(): void;
		\u0063?: number;
	}
</script>
