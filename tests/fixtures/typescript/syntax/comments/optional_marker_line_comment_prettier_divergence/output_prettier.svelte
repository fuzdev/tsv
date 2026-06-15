<script lang="ts">
	interface I {
		a?: number; // c1
	}

	type T = {
		b?: number; // c2
	};

	class A {
		c?: number = 1; // c3
	}
</script>
