<script lang="ts">
	// Function types

	// 100 chars exactly - stays inline (at boundary)
	type A = (  arg1 :  string ,  arg2 :  number ,  arg3 :  boolean ,  arg4 :  object ,  arg5 :  nulllllllllll  ) => string;

	// 101 chars - wraps to one parameter per line
	type B = (
		arg1 :  string  ,
		arg2 :  number  ,
		arg3 :  boolean  ,
		arg4 :  object  ,
		arg5 :  nullllllllllll  ,
	) => string;

	// Constructor types

	// 100 chars exactly - stays inline (at boundary)
	type C = new (  arg1 :  string ,  arg2 :  number ,  arg3 :  boolean ,  arg4 :  object ,  arg5 :  nullllllllllll  ) => A;

	// 101 chars - wraps to one parameter per line
	type D = new (
		arg1 :  string  ,
		arg2 :  number  ,
		arg3 :  boolean  ,
		arg4 :  object  ,
		arg5 :  nulllllllllllll  ,
	) => A;
</script>
