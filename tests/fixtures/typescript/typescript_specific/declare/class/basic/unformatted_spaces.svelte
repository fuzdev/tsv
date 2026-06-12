<script lang="ts">
	declare   class   Empty   {   }


	declare   class   WithConstructor   {

		constructor ( x :  number ) ;
	}


	declare   class   WithProps   {

		prop :  string ;
		readonly   readOnly :  number ;
		static   staticProp :  boolean ;
	}


	declare   class   WithMethods   {

		method (  ) :  void ;
		method2 ( x :  number ) :  string ;
		static   staticMethod (  ) :  void ;
	}


	declare   class   Generic < T >   {

		value :  T ;
	}


	export   declare   class   Exported   {
		
		prop :  string ;
	}
</script>
