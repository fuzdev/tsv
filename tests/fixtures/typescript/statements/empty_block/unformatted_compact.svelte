<script lang="ts">
	// Empty if block (expands)
	if(cond){}

	// Empty else block (expands)
	if(cond){a}else{}

	// Empty while block (stays inline)
	while(cond){

	}

	// Empty for block (stays inline)
	for(;;){

	}

	// Empty for-in block (expands)
	for(const x in y){}

	// Empty for-of block (expands)
	for(const x of y){}

	// Empty do-while block (stays inline)
	do{

	}while(cond);

	// Empty try block (expands), catch (stays inline) when no finally follows
	try{

	}catch(e){

	}

	// Empty finally block (expands)
	try{a;}finally{}

	// Empty catch expands too when a finally follows (not the last clause)
	try{

	}catch(e){

	}finally{

	}

	// Same for a parameterless catch
	try{

	}catch{

	}finally{

	}

	// Empty function body (stays inline)
	function fn(){

	}

	// Empty arrow function body (stays inline)
	const f=()=>{

	};

	// Empty class method body (stays inline)
	class C{m(){

	}}

	// Bare block statement (expands)
	{}

	// Labeled block statement (expands)
	lbl:{}

	// Nested bare block inside a block (expands)
	{{}}
</script>
