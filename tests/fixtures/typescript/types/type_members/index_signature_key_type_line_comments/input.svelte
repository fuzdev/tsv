<script lang="ts">
	// Interface union key: comments break (none merge), the type indents one level
	interface A {
		[
			k: // a
				// b
				string | number
		]: boolean;
	}

	// Interface pre-colon comment forces a space before `:`; key-type comments break, stay flush
	interface B {
		[
			k /* x */ : // c
			// d
			string
		]: boolean;
	}

	// Interface plain key: comments break onto their own lines, stay flush
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
			k: // g
				// h
				string | number
		]: boolean;
	};

	// Type-literal plain key: comments break, stay flush
	type E = {
		[
			k: // i
			// j
			string
		]: boolean;
	};
</script>
