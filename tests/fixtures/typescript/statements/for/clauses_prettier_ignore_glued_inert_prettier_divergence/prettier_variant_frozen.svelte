<script lang="ts">
	// a glued directive is an ordinary comment: the clause formats normally
	for (/* prettier-ignore */ i  =  0; i < 10; i++) {
		fn();
	}

	function fn1() {
		// the same at a restricted production's grouping `(`
		return /* prettier-ignore */ a  +  b;
	}

	// and at a condition head's `(`
	if (/* prettier-ignore */ a  +  b) {
		fn();
	}
</script>
