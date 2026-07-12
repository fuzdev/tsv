<script lang="ts">
	// Integer above i64::MAX (9223372036854775807) but within u64::MAX
	// (18446744073709551615) — emitted through the u64 branch of the number writer.
	const a = 10000000000000000000;
	const b = 18000000000000000000;
</script>
