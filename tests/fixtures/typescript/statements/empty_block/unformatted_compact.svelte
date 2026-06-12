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

	// Empty try block (expands), catch (stays inline)
	try{

	}catch(e){

	}

	// Empty finally block (expands)
	try{a;}finally{}

	// Empty function body (stays inline)
	function fn(){

	}

	// Empty arrow function body (stays inline)
	const f=()=>{

	};

	// Empty class method body (stays inline)
	class C{m(){
		
	}}
</script>
