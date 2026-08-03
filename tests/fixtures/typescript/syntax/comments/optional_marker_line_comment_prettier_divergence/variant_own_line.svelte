<script lang="ts">
	interface I {
		a?: number; // c1

		// Two comments around the member stay distinct (prettier merges them — loss).
		g?: number; // c4 // c5
	}

	type T = {
		b?: number; // c2
	};

	class A {
		c?: number = 1; // c3
	}
</script>
