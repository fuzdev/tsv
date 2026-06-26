<script lang="ts">
	interface I {
		a? // c1
			: number;

		// Two comments around the member stay distinct (prettier merges them — loss).
		g? // c4
			: number; // c5
	}

	type T = {
		b? // c2
			: number;
	};

	class A {
		c? // c3
			: number = 1;
	}
</script>
