<style>
	div {
		grid-template-columns: repeat(3, 1fr);
		grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
		grid-template-columns: repeat(auto-fit, 100px);
		grid-template-columns: [start] 1fr [middle] 1fr [end];
		grid-template-rows: [header-start] auto [header-end main-start] 1fr [main-end];
		grid-template-columns: minmax(100px, 1fr);
		font-size: clamp(1rem, 2vw, 2rem);
	}
</style>
