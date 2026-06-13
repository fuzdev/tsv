<style>
	div {
		/* Quoted URLs - single quotes */
		background: url(
			'image.png'
		);
		background-image: url(
			'../assets/bg.jpg'
		);
		/* Quoted URLs - double quotes */
		border-image: url(
			'font.woff'
		);
		mask-image: url(
			'https://example.fuz.dev/image.png'
		);
		/* Unquoted URLs */
		list-style-image: url(
			image.png
		);
		content: url(
			../assets/bg.jpg
		);
		cursor: url(
			/absolute/path.svg
		),
			auto;
		/* Data URIs - unquoted */
		background-color: url(
			data:image/png;base64,ABC123
		);
		background-position: url(
			data:text/plain;charset=utf-8,Hello%20World
		);
		/* Data URIs - quoted (commas inside quotes) */
		background: url(
			'data:image/svg+xml,<svg></svg>'
		);
		background: url(
			'data:text/css,body{color:red}'
		);
		/* URLs with spaces (must be quoted) */
		outline: url(
			'file with spaces.png'
		);
		border: url(
			'path to file.jpg'
		);
	}

	@font-face {
		font-family: 'Custom';
		src: url(
			'font.woff2'
		) format(
			'woff2'
		);
	}
</style>
