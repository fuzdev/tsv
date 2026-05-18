<script lang="ts">
	// Line comment between name and extends
	interface A // comment
		extends B {
		x: number;
	}

	// Line comment between type params and extends
	interface C<T> // comment
		extends B {
		x: T;
	}

	// Block comment between name and extends
	interface D /* comment */ extends B {
		x: number;
	}

	// Multiple extends with comment
	interface E // comment
		extends B, C {
		x: number;
	}
</script>
