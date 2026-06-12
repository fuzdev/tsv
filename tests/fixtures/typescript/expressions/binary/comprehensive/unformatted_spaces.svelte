<script lang="ts">
	// Arithmetic operators
	let  add  =  a  +  b ;
	let  sub  =  a  -  b ;
	let  mul  =  a  *  b ;
	let  div  =  a  /  b ;
	let  mod  =  a  %  b ;
	let  exp  =  a  **  b ;

	// Comparison operators
	let  eq  =  a  ==  b ;
	let  neq  =  a  !=  b ;
	let  seq  =  a  ===  b ;
	let  sneq  =  a  !==  b ;
	let  lt  =  a  <  b ;
	let  gt  =  a  >  b ;
	let  lte  =  a  <=  b ;
	let  gte  =  a  >=  b ;

	// Logical operators
	let  and  =  a  &&  b ;
	let  or  =  a  ||  b ;
	let  nullish  =  a  ??  b ;

	// Bitwise operators
	let  band  =  a  &  b ;
	let  bor  =  a  |  b ;
	let  bxor  =  a  ^  b ;
	let  bnot  =  ~a ;
	let  lshift  =  a  <<  b ;
	let  rshift  =  a  >>  b ;
	let  urshift  =  a  >>>  b ;

	// Assignment operators
	let  x  =  1 ;
	x  +=  1 ;
	x  -=  1 ;
	x  *=  2 ;
	x  /=  2 ;
	x  %=  3 ;
	x  **=  2 ;
	x  &&=  true ;
	x  ||=  false ;
	x  ??=  0 ;
	x  &=  1 ;
	x  |=  1 ;
	x  ^=  1 ;
	x  <<=  1 ;
	x  >>=  1 ;
	x  >>>=  1 ;

	// Unary operators
	let  neg  =  -a ;
	let  pos  =  +a ;
	let  not  =  !a ;
	let  pre  =  ++a ;
	let  post  =  a++ ;
	let  predec  =  --a ;
	let  postdec  =  a-- ;

	// Comma operator
	let  comma  =  ( a ,  b ,  c ) ;

	// Conditional (ternary) operator
	let  tern  =  a  ?  b  :  c ;
	let  nested  =  a  ?  ( b  ?  c  :  d )  :  e ;

	// typeof, void, delete
	let  t  =  typeof  a ;
	let  v  =  void  0 ;
	let  d  =  delete  obj.prop ;

	// in and instanceof
	let  inop  =  'prop'  in  obj ;
	let  inst  =  a  instanceof  Array ;
</script>
