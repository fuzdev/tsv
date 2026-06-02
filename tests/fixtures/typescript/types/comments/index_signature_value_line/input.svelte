<script lang="ts">
	// Index signature value: intersection keeps the first type inline after `:`
	type T = {
		[k: string]: A & // c
			B;
	};

	// Multiple members
	type U = {
		[k: string]: A & // c
			B & // d
			C;
	};
</script>
