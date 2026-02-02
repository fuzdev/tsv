<script lang="ts">
	// Chained method calls with callbacks - should hug
	aaaaa
		.bbbbb('ccccc', () => {
			ddddd.eeeee(fffff);
		})
		.bbbbb('ggggg', () => {
			hhhhh.iiiii(jjjjj);
		});

	// Single call for comparison - also hugs
	aaaaa.bbbbb('kkkkk', () => {
		lllll.mmmmm(nnnnn);
	});
</script>
