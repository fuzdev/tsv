<style>
	@media screen and (min-width: 768px) and (max-width: 1024px) {
		div {
			display: flex;
		}
	}
	@media (min-width: 768px), (max-width: 1024px) {
		span {
			color: blue;
		}
	}
	@media (min-width: 768px) or (orientation: portrait) {
		h1 {
			font-size: 2rem;
		}
	}
	@media not all and (min-width: 768px) {
		p {
			font-size: 14px;
		}
	}
	@media (min-width: 768px) and (orientation: landscape), (min-width: 1024px) {
		section {
			padding: 2rem;
		}
	}
</style>
