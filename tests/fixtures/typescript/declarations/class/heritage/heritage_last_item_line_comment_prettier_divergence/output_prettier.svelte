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

	// Multiple line comments after extends — each kept on its own line, not merged
	class F extends C {
		// c1
		// c2
	}

	// Multiple line comments after implements (group-broken heritage)
	class G implements I, J {
		// c1
		// c2
	}

	// Multiple line comments after extends (class expression)
	const h = class extends C {
		// c1
		// c2
	};

	// Line then block comment after extends — boundary preserved, each on its own line
	class K extends C {
		// c1
		/* c2 */
	}

	// Multiple line comments after interface extends, each on its own line
	interface M extends I, J {
		// c1
		// c2
	}
</script>
