<script lang="ts">
	// Method signature with leading union return type in nested object type
	// Bug: at 2+ levels of nesting, our formatter keeps "=> |" on same line
	// and fails to properly indent the second union member

	// At exactly 100 chars - union stays inline (no wrapping needed)
	type  T100  =  {

		a? :  {

			b? :  (

				c :  D ,

			)  =>  Eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee[ 'f' ]  |  Promise< Eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee[ 'f' ] > ;

		} ;

	} ;

	// At 101 chars - union wraps to leading form (THIS IS THE BUG)
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
