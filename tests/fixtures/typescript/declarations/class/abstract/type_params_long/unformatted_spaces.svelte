<script lang="ts">
	// 100 chars - stays inline (at boundary)
	abstract  class  Inline < T  extends  TypeAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa > {  }

	// 101 chars - wraps (exceeds print_width)
	abstract  class  Wraps <
		T  extends  TypeAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ,
	> {  }

	// Long type params with extends heritage - params break, extends inline
	abstract  class  WithExtends <
		T  extends  VeryLongTypeName  |  AnotherLongTypeName ,
		U  extends  ExtraLongConstraintType  |  MoreTypes ,
	>  extends  Base < T > {
		abstract  value :  T ;
	}

	// Long type params with implements - implements breaks, brace on new line
	abstract  class  WithImplements <
		T  extends  VeryLongTypeName  |  AnotherLongTypeName ,
		U  extends  ExtraLongConstraintType  |  MoreTypes ,
	>
		implements  Contract < T > ,  Contract < U >
	{
		abstract  a :  T ;
		abstract  b :  U ;
	}

	// Long with both extends and implements - all break
	abstract  class  WithBoth <
		T  extends  VeryLongTypeName  |  AnotherLongTypeName ,
		U  extends  ExtraLongConstraintType  |  MoreTypes ,
	>
		extends  Base < T >
		implements  Contract < U >
	{
		abstract  prop :  string ;
	}

	// Short - stays on one line (contrast case)
	abstract  class  Short < T  extends  string  =  string ,  U  extends  number  =  number > {
		abstract  a :  T ;
		abstract  b :  U ;
	}
</script>
