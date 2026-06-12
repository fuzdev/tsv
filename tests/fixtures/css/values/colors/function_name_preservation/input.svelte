<style>
	div {
		/* hsla without alpha - function name must be preserved */
		color: hsla(0, 100%, 50%);
		background: hsla(120, 100%, 50%);
		/* hsla with slash alpha - function name must be preserved */
		color: hsla(0 100% 50% / 0.5);
		background: hsla(120 100% 50% / 0.3);
		/* rgba without alpha - function name must be preserved */
		color: rgba(255, 0, 0);
		background: rgba(0, 255, 0);
		/* rgba with slash alpha - function name must be preserved */
		color: rgba(255 0 0 / 0.5);
		background: rgba(0 255 0 / 0.3);
	}
</style>
