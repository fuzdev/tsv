<script lang="ts">
	// A block comment trailing a mapped type without a value type stays where
	// the author wrote it — after the `]` (or the optional modifier)
	type A = { [K in B] /* c */ };
	type C = { [K in B]? /* c */ };
</script>
