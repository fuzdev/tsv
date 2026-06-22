<script lang="ts">
	// Async arrows with semicolons WORK (regression test)
	const f1 = async (): { a: number; b: number } => ({ a: 1, b: 2 });
	const f2 = async <T extends object>(): { a: number; b: number } => ({ a: 1, b: 2 });
	const f3 = async (x: number): { a: number; b: number } => ({ a: x, b: x });
</script>
