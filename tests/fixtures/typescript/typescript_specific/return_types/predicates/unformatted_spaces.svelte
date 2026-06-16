<script lang="ts">
	// Function declaration with type predicate
	function  isStr ( x :  unknown ) :  x  is  string  {
		return  typeof  x  ===  'string' ;
	}

	function  isNum ( x :  unknown ) :  x  is  number  {
		return  typeof  x  ===  'number' ;
	}

	function  isNonNull < T > ( x :  T  |  null ) :  x  is  T  {
		return  x  !==  null ;
	}

	// Arrow function with type predicate
	const  isBool  =  ( x :  unknown ) :  x  is  boolean  =>  typeof  x  ===  'boolean' ;

	const  isArr  =  < T  extends  object > ( x :  unknown ) :  x  is  T [ ]  =>  Array.isArray ( x ) ;

	// Method with type predicate
	class  Guard  {
		isObj ( x :  unknown ) :  x  is  object  {
			return  typeof  x  ===  'object'  &&  x  !==  null ;
		}

		static  isFn ( x :  unknown ) :  x  is  Function  {
			return  typeof  x  ===  'function' ;
		}
	}

	// Object method shorthand
	const  guards  =  {
		isUndef ( x :  unknown ) :  x  is  undefined  {
			return  x  ===  undefined ;
		} ,
	} ;

	// Type predicate with complex type
	function  isStrArr ( x :  unknown ) :  x  is  string [ ]  {
		return  Array.isArray ( x )  &&  x.every ( ( i )  =>  typeof  i  ===  'string' ) ;
	}

	// Type predicate with union narrowing
	function  isA ( x :  A  |  B ) :  x  is  A  {
		return  'a'  in  x ;
	}

	// this is T predicate (narrows this)
	class  Box < T >  {
		value :  T  |  null  =  null ;

		hasValue ( ) :  this  is  { value :  T }  {
			return  this.value  !==  null ;
		}
	}

	// Overloaded function with type predicate
	function  check ( x :  unknown ) :  x  is  string ;
	function  check ( x :  unknown ) :  x  is  number ;
	function  check ( x :  unknown ) :  x  is  string  |  number  {
		return  typeof  x  ===  'string'  ||  typeof  x  ===  'number' ;
	}
</script>
