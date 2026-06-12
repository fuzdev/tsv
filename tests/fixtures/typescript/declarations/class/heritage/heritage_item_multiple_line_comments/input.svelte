<script lang="ts">
	// Multiple line comments between implements items
	class A
		implements
			I, // c1
			// c2
			J {}

	// Multiple line comments with extends
	class B
		extends C
		implements
			I, // c1
			// c2
			J {}

	// Three line comments between items
	class C
		implements
			I, // c1
			// c2
			// c3
			J {}
</script>
