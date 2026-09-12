<script lang="ts">
	// an own-line directive in a sole parameter's `:`→type gap freezes the type, and the
	// parameter list breaks rather than hugging the object type
	function f(p: (
		// prettier-ignore
		{x:   1}
	)) {}

	// the same at a function type's parameter list
	type A = (q: (
		// prettier-ignore
		{x:   2}
	)) => void;

	// a method signature's sole parameter
	type B = {
		m(r: (
			// prettier-ignore
			{x:   3}
		)): void;
	};
</script>
