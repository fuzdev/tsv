<script lang="ts">
	type A = { validId: number; 'needs-quote': string; café: boolean; foo(): void };
	interface I {
		plainId: number;
		'needs-quote': string;
		bar(): void;
	}
</script>
