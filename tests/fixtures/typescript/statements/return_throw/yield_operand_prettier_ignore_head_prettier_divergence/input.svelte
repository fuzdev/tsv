<script lang="ts">
	function* fn1() {
		// `yield` shares the restricted-production freeze with `return` / `throw`, and
		// keeps the hanging-paren layout the comment forces
		yield (
			// prettier-ignore
			a  +  b
		);
	}

	function* fn2() {
		// `yield*` classifies the same way
		yield* (
			// prettier-ignore
			a  +  b
		);
	}
</script>
