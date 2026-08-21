<script  lang="js" >
let   a   =   1;
</script>

<svelte:head>
	<script  type="module" >
		import  {  b  }  from  './b.js';
		b(  1,  2  );
	</script>
</svelte:head>

<div>
	<script  lang="typescript" >
		const  c  :  number   =   3;
	</script>
	<script  type="application/javascript" >
		const   d   =   4;
	</script>
	<script  type="text/ecmascript" >
		const   e   =   5;
	</script>
	<script  type="application/ecmascript" >
		const   f   =   6;
	</script>
</div>
