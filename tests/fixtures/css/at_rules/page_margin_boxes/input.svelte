<style>
	@page {
		size: A4;
		margin: 2cm;

		@top-left {
			content: 'Header Left';
		}

		@top-center {
			content: 'Header Center';
		}

		@bottom-right {
			content: counter(page);
		}
	}
</style>
