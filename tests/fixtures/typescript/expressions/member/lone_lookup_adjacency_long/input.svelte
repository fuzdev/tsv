<script lang="ts">
	// The same clause's object operand, read literally: the lookup's object must be a bare
	// identifier, so the lookup has to sit immediately after the base. A `!` or a `?.` on the
	// OTHER side is a chain-element wrapper rather than a member, and neither disqualifies it.

	class Q {
		m(objx: Q) {
			// the object is a computed lookup, not an identifier
			objx[iii]
				.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;

			// the object is a non-null assertion - at exactly 100 the weld would still fit,
			// but the break point is there either way, so only width decides
			objx!.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;

			// one char more, and the lookup takes its own line
			objx!
				.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;

			// a trailing `!` is a wrapper, so the lookup keeps its line past width
			objx.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa!;

			// and so is `?.`
			objx?.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;
		}
	}
</script>
