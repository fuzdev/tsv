<script lang="ts">
	// Line comment after last implements item before body
	class A implements I, J {
		// c
	}

	// Line comment after last extends before body
	class B extends C {
		// c
	}

	// Line comment after last extends item before body (interface)
	interface D extends I, J {
		// c
	}

	// Line comment after last implements with extends before body
	class E extends C implements I, J {
		// c
	}
</script>
