<script lang="ts">
	// array: blank line after an own-line comment is preserved
	const arr = [


		x,
		/* a */



		y


	];

	// call args: blank after own-line comment
	fn(


		x,
		/* b */



		y


	);

	// type parameters: blank after own-line comment
	function f<


		A,
		/* c */



		B


	>() {}

	// tuple: blank before and after own-line comment
	type Tup = [


		A,



		/* d */



		B


	];

	// enum: blank after own-line comment
	enum E {


		A,
		/* e */



		B


	}

	// variable declarators: blank after own-line comment
	const v1 = 1,
		/* f */



		v2 = 2;
</script>
