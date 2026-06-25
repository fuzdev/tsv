<script lang="ts">
	interface I {
		a?: number; // c1

		// Two comments around the member stay distinct (prettier merges them — loss).
		g?: number; // c5 // c6
	}

	type T = {
		b?: number; // c2
	};

	class A {
		c?: number; // c3
	}

	interface M {
		fn?(): void; // c4
	}
</script>
