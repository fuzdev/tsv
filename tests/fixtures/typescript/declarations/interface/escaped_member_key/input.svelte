<script lang="ts">
	// Interface member keys decode from escapes (also covers type-literal keys: same parser path)
	interface I {
		in: string;
		b(): void;
		c?: number;
	}
</script>
