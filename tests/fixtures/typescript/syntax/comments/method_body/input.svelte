<script lang="ts">
	// Comment authored at the start of a method body stays there
	class A {
		fn() {
			// comment
			return 1;
		}
	}

	const obj = {
		fn() {
			// comment
			return 1;
		}
	};
</script>
