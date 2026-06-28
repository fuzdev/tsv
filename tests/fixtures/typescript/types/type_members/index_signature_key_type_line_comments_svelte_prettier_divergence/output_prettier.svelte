<script lang="ts">
	// Interface union key: comments break (none merge), the type indents one level
	interface A {
		[
			k:
				// a
				// b
				string | number
		]: boolean;
	}

	// Interface pre-colon comment forces a space before `:`; key-type comments indent one level
	interface B {
		[
			k /* x */ : // c
			// d
			string
		]: boolean;
	}

	// Interface plain key: comments and type indent one level
	interface C {
		[
			k: // e
			// f
			string
		]: boolean;
	}

	// Type-literal union key: same as the interface union case (separate printer)
	type D = {
		[
			k:
				// g
				// h
				string | number
		]: boolean;
	};

	// Type-literal plain key: comments and type indent one level
	type E = {
		[
			k: // i
			// j
			string
		]: boolean;
	};
</script>
