<style>
	div {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		grid-template-rows: auto 1fr auto;
		grid-template-areas:
			'header header'
			'sidebar main'
			'footer footer';
		grid-auto-rows: minmax(100px, auto);
		grid-auto-columns: 1fr;
		grid-auto-flow: dense;
		gap: 1rem 2rem;
		row-gap: 0.5rem;
		column-gap: 1.5rem;
	}
</style>
