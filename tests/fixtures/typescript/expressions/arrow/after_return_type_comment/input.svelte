<script lang="ts">
	// Block comment after return type - stays in place
	const a = (x: string): number /* block */ => 1;
	const b = (): void /* block */ => {};
	const c = async (): Promise<void> /* block */ => {};
</script>
