<script lang="ts">
	// Arrow expression body with call that wraps - chain in callee
	a(  ).b(  (  )    =>

		fn(  {
			a:    1,
		}  ),
	);

	// Longer chain
	obj.method1(   arg  ).
	method2((  )  =>

		Promise
		.resolve( {
			isA:
			  (  )  => false,
			isB:(  )   =>  
			true,
		
		}    as any ),
	);

	// With assignment
	const   x   =   a( ).b((  ) =>

		fn(  {
			prop   :    value,
		}),
	);

	// Contrasting case: block body stays hugged (matches)
	a( )
	.b((  )   =>  {

		return fn(  {
			a   :1,
		});
	});

	// Contrasting case: parenthesized object return stays hugged (matches)
	a(  ).b(
		( 

	)   =>   
	({
		a :  1,
	}));
</script>
