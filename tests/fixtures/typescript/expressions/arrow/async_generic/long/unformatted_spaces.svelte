<script lang="ts">
	// Short - stays on one line (contrast case)
	const  short  =  async  < T  extends  object > (  )  =>  {  }  ;

	// Long constraint - type params wrap
	const  longConstraint  =  async  <
		T  extends  VeryLongConstraintTypeName  |  AnotherLongConstraintTypeName  ,
	> (  )  =>  {  }  ;

	// Multiple long type params - each on own line
	const  multipleLong  =  async  <
		T  extends  LongConstraintOne  |  LongConstraintTwo  ,
		U  extends  LongConstraintThree  |  LongConstraintFour  ,
	> (  )  =>  {  }  ;

	// Long with constraint and default - wraps
	const  longWithDefault  =  async  <
		T  extends  VeryLongConstraintTypeName  =  VeryLongDefaultTypeName  ,
	> (  )  =>  {  }  ;

	// Long with parameters and return type
	const  longWithParams  =  async  <
		T  extends  VeryLongConstraintTypeName  |  AnotherLongConstraintTypeName  ,
	> (
		param  :  T  ,
	)  :  Promise < T >  =>  param  ;

	// Long object type constraint - wraps
	const  longObjectConstraint  =  async  <
		T  extends  {
			longPropertyName  :  VeryLongTypeName  ;
			anotherLongProperty  :  AnotherLongTypeName  ;
		}  ,
	> (  )  =>  {  }  ;

	// Long function type constraint - wraps
	const  longFnConstraint  =  async  <
		T  extends  (
			longParam  :  VeryLongParamType  ,
			anotherParam  :  AnotherLongParamType  ,
		)  =>  Promise < VeryLongReturnType >  ,
	> (  )  =>  {  }  ;

	// Long with rest parameter - stays inline (just under 100 chars)
	const  longWithRest  =  async  < T  extends  VeryLongConstraintTypeName  |  AnotherLongConstraintTypeName > (
		...args  :  T [  ]
	)  :  Promise < T [  ] >  =>  args  ;

	// Long conditional type in default - wraps
	const  longConditionalDefault  =  async  <
		T  ,
		U  extends  T  =  T  extends  VeryLongConditionType  ?  VeryLongTrueType  :  never  ,
	> (  )  =>  {  }  ;
</script>
