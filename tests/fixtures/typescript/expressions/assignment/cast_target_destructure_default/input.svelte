<script lang="ts">
	// A cast target with a default in a destructuring pattern unwraps to the
	// plain target (and drops the parens), like acorn's toAssignable.
	({ a: (b as T) = 1 } = obj);
	({ a: (b satisfies T) = 2 } = obj);
	({ a: (<T>b) = 3 } = obj);
	({ a: b! = 4 } = obj);
	[(c as T) = 5] = arr;
	// The same unwrap applies inside a no-declaration for-of/for-in head.
	for ([(c as T) = 6] of arr) {
	}
</script>
