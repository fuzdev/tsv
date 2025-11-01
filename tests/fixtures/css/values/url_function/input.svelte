<style>
	/* Test url() function values */
	.background-image {
		background: url('image.png');
	}

	.unquoted-url {
		background-image: url(image.png);
	}

	.complex-path {
		background: url('../images/bg.jpg');
	}

	.font-face-url {
		font-family: 'Custom';
		src: url('font.woff2') format('woff2');
	}
</style>
