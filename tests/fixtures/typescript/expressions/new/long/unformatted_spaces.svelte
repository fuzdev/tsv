<script lang="ts">
	// Short new (contrast case)
	new  Map (  )  ;

	// 100 chars - stays inline (at boundary)
	new  VeryLongConstructorClassName000000000000000000000000000000000000000000000000000000000000000 (  )  ;

	// 101 chars - still inline for new expressions (no natural break point)
	new  VeryLongConstructorClassName0000000000000000000000000000000000000000000000000000000000000000 (  )  ;

	// Long constructor with generic types
	new  Map <
		VeryLongKeyTypeName000000000000000000000  ,
		VeryLongValueTypeName000000000000000000000000
	> (  )  ;

	// Long constructor arguments - wrap
	new  VeryLongConstructorClassName (
		veryLongArgumentOne  ,
		veryLongArgumentTwo  ,
		veryLongArgumentThree  ,
		veryLongArgumentFour  ,
	)  ;

	// Long generic constructor with arguments - type params wrap
	new  GenericClass < VeryLongTypeParameterOne00000000000 ,  VeryLongTypeParameterTwo00000000000 > (
		veryLongArgumentOne  ,
		veryLongArgumentTwo  ,
	)  ;

	// Chained new with long method calls
	new  VeryLongConstructorClassName0 (  )
		.veryLongMethodName (  )
		.anotherVeryLongMethodName (  )
		.finalMethod (  )  ;

	// Long new in variable declaration
	const  instance  =  new  VeryLongConstructorClassName (
		veryLongArgumentOne  ,
		veryLongArgumentTwo  ,
		veryLongArgumentThree  ,
	)  ;

	// Long new with object literal argument
	new  VeryLongConstructorClassName (  {
		veryLongPropertyOne  :  valueOne  ,
		veryLongPropertyTwo  :  valueTwo  ,
		veryLongPropertyThree  :  valueThree  ,
	}  )  ;

	// Long new with callback argument
	new  VeryLongConstructorClassName0000000000000000000000000000000000 (
		(  veryLongCallbackParameter  )  =>  {
			return  veryLongCallbackParameter.transform (  )  ;
		}  ,
	)  ;

	// Nested long new expressions
	new  OuterVeryLongConstructorClassName (
		new  InnerVeryLongConstructorClassName (
			veryLongArgumentOne000000000  ,
			veryLongArgumentTwo000000000  ,
		)  ,
	)  ;
</script>
