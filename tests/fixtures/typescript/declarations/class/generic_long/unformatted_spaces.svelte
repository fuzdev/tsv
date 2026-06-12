<script lang="ts">
	// Simple type parameters

	// 100 chars - stays inline (at boundary)
	class AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA< 
	 T  ,  U  > {}

	// 101 chars - wraps (exceeds print_width)
	class AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA<
		T  ,
		U  ,
	> {}

	// Type parameters with extends constraint

	// 100 chars - stays inline (at boundary)
	class Inline<  
	T   extends  
	 string | number | boolean  ,  U   extends   Record<string, unknown___________>  > {}

	// 101 chars - wraps (exceeds print_width)
	class Wraps<
		T   extends   string | number | boolean  ,
		U   extends   Record<string, unknown_____________>  ,
	> {}

	// Type parameters with defaults

	// 100 chars - stays inline (at boundary)
	class InlineDefaults<  T   =  
	 string  ,  U   extends   T   =   T  ,  W   =  
	  Record<string, unknown_________________>  > {}

	// 101 chars - wraps (exceeds print_width)
	class WrapsDefaults<
		T   =   string  ,
		U   extends   T   =   T  ,
		W   =   Record<string, unknown___________________>  ,
	> {}

	// Named class expression

	// 100 chars - stays inline (at boundary)
	const Ei = class Inner<  T   extends   string
	 |
	  number | boolean  ,  
		U   extends   Record<
		string, un_____> 
		 > {

		 };

	// 101 chars - wraps (exceeds print_width)
	const Ew = class Inner<
		T   extends   string | number | boolean  ,
		U   extends   Record<string, un______>  ,
	> {};

	// Anonymous class expression

	// 100 chars - stays inline (at boundary)
	const AnonInline = class< 
	 T   extends   string | number | boolean  ,
	   U   extends   Record<string, u____>  > 
		 {
			
		 };

	// 101 chars - wraps (exceeds print_width)
	const AnonWraps = class<
		T   extends   string | number | boolean  ,
		U   extends   Record<string, u______>  ,
	> {};
</script>
