<script lang="ts">
	// variable initializer: blank after an own-line leading comment is preserved
	const a =


		/* a */



		1;

	// object property value: same rule
	const o = {
		b:


			/* b */



			2
	};

	// class field initializer: same rule
	class C {
		c =


			/* c */



			3;
	}

	// call argument: same rule
	fn(


		/* d */



		arg
	);

	// line comment leading the value
	const e =


		// e



		5;

	// arrow expression body: same rule
	const f = () =>


		/* f */



		6;

	// unary operand (paren-wrapped to hold the comment): same rule
	const g = !(


		/* g */



		h
	);
</script>
