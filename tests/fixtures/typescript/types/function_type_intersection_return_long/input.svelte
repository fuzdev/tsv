<script lang="ts">
	// Intersection return type in function type - 100/101 boundary
	// Bug: at 2+ levels of nesting, continuation wasn't indented properly

	// At exactly 100 chars - stays inline
	type T100 = {
		a?: {
			b?: (
				c: D,
			) => Eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee['f'] & Promise<Eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee['f']>;
		};
	};

	// At 101 chars - wraps with trailing &
	type T101 = {
		a?: {
			b?: (
				c: D,
			) => Eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee['f'] &
				Promise<Eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee['f']>;
		};
	};
</script>
