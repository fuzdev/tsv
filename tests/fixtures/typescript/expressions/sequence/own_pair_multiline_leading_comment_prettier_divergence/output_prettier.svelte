<script lang="ts">
	// a multi-line comment the author wrote inside a sequence's own pair stays inside it
	export default /* c
	 * d */ (b, c);

	// the same sequence as a later operand of another sequence
	x =
		(a,
		/* c
		 * d */ (b, c));
</script>
