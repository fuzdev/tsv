<script lang="ts">
	// A should-hug union whose object-like member is a type REFERENCE
	// (`Map<…> | null`) never breaks after the operator: the reference's type
	// arguments own the break, exactly as the object member owns its expansion in
	// `{ … } | null`. Uniform across the annotation, `is`, and `=>` seams.

	// Variable: 100 chars stays inline, 101 breaks the type arguments
	let aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:Map<string,number>|null;
	let aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:Map<string,number>|null;

	// Class property
	class Single{aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:Map<string,number>|null;}

	// Parameter: 100 chars stays inline, 101 breaks the type arguments
	function fn100(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:Map<string,number>|null){}
	function fn101(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:Map<string,number>|null){}

	// Index signature
	interface Multiple{[aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:string]:Map<string,number>|null;}

	// Return types: function, arrow, and a type predicate
	function ret():Promiseaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa<string,number>|null{}
	const arrow=():Promiseaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa<string,number>|null=>{};
	function pred(a:unknown):a is Promiseaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa<string,number>|null{}

	// Function-type return: 100 chars stays inline, 101 breaks the type arguments
	type A=()=>Promiseaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa<string,number>|null;
	type B=()=>Promiseaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa<string,number>|null;
</script>
