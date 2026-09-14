<script lang="ts">
	// The join is the two tokens, not the operand's node: a `typeof` argument that
	// merely ENDS on the close re-lexes exactly as the bare instantiation does.
	const a = typeof f<T> > 1;
</script>
