<script lang="ts">
	// Short - stays on one line (contrast case)
	const  short  =  < T  extends  string > (  x  :  T  )  :  T  =>  x  ;

	// Long single type param - stays inline, params break
	const  longConstraint  =  < T  extends  VeryLongConstraintTypeName  |  AnotherLongConstraintTypeName > (
		x  :  T  ,
	)  :  T  =>  x  ;

	// Multiple long type params - each on own line
	const  multipleLong  =  <
		T  extends  LongConstraintOne  |  LongConstraintTwo  ,
		U  extends  LongConstraintThree  |  LongConstraintFour  ,
	> (
		x  :  T  ,
		y  :  U  ,
	)  :  [  T  ,  U  ]  =>  [  x  ,  y  ]  ;

	// Long with constraint and default - stays inline
	const  longWithDefault  =  < T  extends  VeryLongConstraintTypeName  =  VeryLongDefaultTypeName > (
		x  :  T  ,
	)  :  T  =>  x  ;

	// Long object type constraint - wraps to multiple lines
	const  longObjectConstraint  =  <
		T  extends  {
			longPropertyName  :  VeryLongTypeName  ;
			anotherLongProperty  :  AnotherLongTypeName  ;
		}  ,
	> (
		x  :  T  ,
	)  :  T  =>  x  ;

	// Long function type constraint - wraps
	const  longFnConstraint  =  <
		T  extends  (
			longParam  :  VeryLongParamType  ,
			anotherParam  :  AnotherLongParamType  ,
		)  =>  VeryLongReturnType  ,
	> (
		f  :  T  ,
	)  :  T  =>  f  ;

	// Long conditional type in default - wraps
	const  longConditionalDefault  =  <
		T  ,
		U  extends  T  =  T  extends  VeryLongConditionType  ?  VeryLongTrueType  :  never  ,
	> (
		x  :  T  ,
	)  =>  x  ;
</script>
