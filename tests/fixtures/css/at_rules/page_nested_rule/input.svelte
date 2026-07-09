<style>
	/* @page block is agnostic: a nested style rule parses alongside declarations */
	@page :first {
		h1 {
			color: red;
		}
	}

	/* declarations, margin at-rules, and nested rules coexist in one @page block */
	@page {
		margin: 1cm;
		@top-center {
			content: 'x';
		}
		h1 {
			color: blue;
		}
	}
</style>
