<script lang="ts">
	interface I {
		m(): void; // c1
		(): void; // c2
		new (): void; // c3
		k(): void /* d */;
	}
</script>
