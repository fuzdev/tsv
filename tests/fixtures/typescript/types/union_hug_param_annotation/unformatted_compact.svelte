<script lang="ts">
	// A brace-hugging union hugs a parameter's `:` — the object owns its expansion,
	// the void member trails the `}` — exactly as it does at a variable or property.

	// Sole parameter: the list breaks, the annotation hugs
	function sole(a:{
b:1;
}|null){}

	// With a sibling parameter
	function sibling(a:{
b:1;
}|null,c:string){}

	// Optional parameter
	function optional(a?:{
b:1;
}|null,c?:string){}

	// Destructured parameter
	function destructured({
x
}:{
x:1;
}|null,c:string){}

	// Null first, and a void mix
	function nullFirst(a:null|{
b:1;
},c:string){}
	function voidMix(a:{
b:1;
}|null|void,c:string){}

	// Arrow and class method parameters
	const arrow=(a:{
b:1;
}|null,c:string)=>{};
	class Single{method(a:{
b:1;
}|null,c:string){}}

	// Index signature value
	interface Multiple{[key:string]:{
b:1;
}|null;}

	// A non-hugging union still breaks after the `:`
	function nonHug(a:{
b:1;
}|boolean,c:string){}
</script>
