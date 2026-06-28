<script lang="ts">
	// Method signature with a union return type in a nested object type.
	// The union breaks after `=>` onto an indented continuation line and only
	// stays inline when it fits.

	// At exactly 100 chars - union stays inline (no wrapping needed)
	type  T100  =  {

		a? :  {

			b? :  (

				c :  D ,

			)  =>  Eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee[ 'f' ]  |  Promise< Eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee[ 'f' ] > ;

		} ;

	} ;

	// At 101 chars - union breaks after `=>` to an indented continuation line
	type  T101  =  {

		a? :  {

			b? :  (

				c :  D ,

			)  =>

				|  Eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee[ 'f' ]

				|  Promise< Eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee[ 'f' ] > ;

		} ;

	} ;
</script>
