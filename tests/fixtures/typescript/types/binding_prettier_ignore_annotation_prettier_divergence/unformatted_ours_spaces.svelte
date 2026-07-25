<script lang="ts">
	// an own-line directive before a binding's `:` stays own-line and freezes the whole
	// `: type` annotation — class property, parameter, variable, index-signature key
	class A {
		a
				// prettier-ignore
				: {x:   1} =  { x: 1 };

		// before the `?` marker: the freeze starts at the marker, the text the directive
		// precedes
		b
				// prettier-ignore
				?: {w:   4};
	}

	function fn(
		b
				// prettier-ignore
				: Record<string,   number>
	) {
		return  b;
	}

	const c
			// prettier-ignore
			: {z:   3} =  { z: 3 };

	interface B {
		[
			k
					// prettier-ignore
					: 'a'  |  'b'
		]:  number;
	}
</script>
