<script lang="ts">
	// a directive alone on its line in the `[`→key gap freezes the key parameter
	type A = {
		[
			// prettier-ignore
			key:   string
		]: number;
	};

	// a class index signature, and one behind modifiers
	class B {
		[
			// prettier-ignore
			key:   string
		]: number;
	}

	interface C {
		readonly [
			// prettier-ignore
			key:   string
		]: number;
	}

	// an own-line block comment behaves identically
	type D = {
		[
			/* prettier-ignore */
			key:   string
		]: number;
	};
</script>
