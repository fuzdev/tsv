<script lang="ts">
	// a multiline template body forces a break the hug states cannot measure past, so the
	// argument list expands — `new` answers this the same way a plain call does
	f(1,(x)=>`a
b`);
	new F(1,   (x)=>`a
b`);

	// ...and so does a source-multiline object inside a binary body
	f(1,(x)=>cond&&{
		k:1
	});
	new F(  1,(x)=>cond &&{
		k:1
	});

	// controls — an expandable body still hugs in both printers
	f(1,(x)=>({
		k:1
	}));
	new F(1,(x)=>   ({
		k:1
	}));

	// ...as does a block body, and an arrow chain whose terminal body is an object
	f(1,(x)=>{g();});
	new F(1,(x)=>{ g(); });
	f(1,()=>()=>({
		k:1
	}));
	new F(1,()=>  ()=>({
		k:1
	}));

	// a short non-expandable body needs no break and stays inline in both
	f(1,(x)=>x.y.z);
	new F(1,(x)=>  x.y.z);
</script>
