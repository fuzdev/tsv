<script lang="ts">
	// Type arguments stay on the flat test-call form.
	it<T>('a', () => {});
	it.only<T, U>('b', () => {});

	// The callee -> `<` gap keeps its comment.
	it/* c */ <T>('c', () => {});

	// The flat form keeps them past the print width, like the bare test call.
	it<T>('ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc', () => {});
</script>
