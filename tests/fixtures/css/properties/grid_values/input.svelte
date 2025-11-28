<style>
	div {
		grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
		grid-template-rows: [header-start] auto [header-end main-start] 1fr [main-end];
		grid-template-columns: 1fr repeat(2, minmax(100px, 1fr)) 1fr;
	}
</style>
