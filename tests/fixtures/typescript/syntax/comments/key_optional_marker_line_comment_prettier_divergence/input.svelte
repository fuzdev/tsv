<script lang="ts">
	interface I {
		a // c1
			?: number;

		// Two comments around the member stay distinct (prettier merges them — loss).
		g // c5
			?: number; // c6
	}

	type T = {
		b // c2
			?: number;
	};

	class A {
		c // c3
			?: number;
	}

	interface M {
		fn // c4
			?(): void;
	}
</script>
