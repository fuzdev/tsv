<script lang="ts">
	interface I {
		[k: string]: // c1
			A | B;
	}

	interface J {
		[k: string]: // c2
		A & B;
	}

	type T = {
		[k: string]: // c3
			A | B;
	};

	type U = {
		[k: string]: // c4
		A & B;
	};

	interface K {
		[k: string]: // c5
		number;
	}
</script>
