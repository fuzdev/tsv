<div class="x">x</div>

<style>
	@keyframes a {
		from {
			opacity: 0;
		}
	}
	@keyframes b {
		from {
			opacity: 0;
		}
	}
	.x {
		animation:
			a 1s ease,
			b 2s;
		animation-name: a, b;
	}
</style>
