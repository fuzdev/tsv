<script lang="ts">
	function* fn1() {
		// Block comment between `yield*` and the operand
		yield /* c1 */ * a;

		// Two comments in the same gap keep their order
		yield/* c2 */ /* c3 */* b;
	}
</script>
