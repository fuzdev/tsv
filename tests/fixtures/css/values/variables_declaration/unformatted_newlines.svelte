<style>
	.class1 {
		/* Simple custom properties */
		--color1: red;
		--color2: #ff0000;
		--color3: rgb(
			255
			0
			0
		);

		/* Complex values */
		--spacing1: calc(
			1rem
			+
			5px
		);
		--shadow1: 0
			2px
			4px
			rgba(
				0,
				0,
				0,
				0.1
			);
		--gradient1: linear-gradient(
			to right,
			red,
			blue
		);

		/* Edge cases - all valid per CSS spec */
		--: value1;
		--123: value2;
		---foo: value3;
		--_private: value4;
		--kebab-case: value5;

		/* Using other CSS values */
		--size1: 16px;
		--size2: 1.5rem;
		--percent1: 50%;
		--angle1: 45deg;
		--time1: 0.3s;
	}
</style>
