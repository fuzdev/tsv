<style>
	:root {
		--prop1: #007bff;
		--prop2: #6c757d;
		--prop3: 16px;
		--prop4: 14px;
	}

	.class1 {
		/* Simple var() usage */
		color: var(
			--color1
		);
		background: var(
			--bg1
		);
		font-size: var(--size1);

		/* var() with fallback */
		color: var(
			--color2,
			red
		);
		background: var(
			--bg2,
			#ffffff
		);
		margin: var(--spacing1, 10px);

		/* Nested fallbacks (2 levels) */
		color: var(
			--color3,
			var(
				--color4,
				blue
			)
		);
		padding: var(
			--spacing2,
			var(--spacing3, 1rem)
		);

		/* Multiple var() in one value */
		margin: var(
			--top
		)
			var(--right)
			var(
				--bottom
			)
			var(--left);
		box-shadow: var(--x)
			var(
				--y
			)
			var(--blur)
			var(--color);

		/* var() with complex fallbacks */
		width: var(
			--width1,
			calc(
				100%
				-
				2rem
			)
		);
		color: var(
			--color5,
			rgb(
				255
				0
				0
			)
		);
	}
</style>
