<script lang="ts">
	// a line comment between the type arguments and the `?.` drops the argument list to a
	// continuation line, and the type arguments stay ahead of the `?.`
	T<U> // c1
		?.(a).b;

	a.T<U> // c2
		?.(b).c;
</script>
