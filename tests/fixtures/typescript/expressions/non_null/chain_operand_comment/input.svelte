<script lang="ts">
	// comment between a chain operand and its non-null assertion — preserved, trailing
	const a = p?.q /* c */!;
	const b = a.b /* c */!;
</script>
