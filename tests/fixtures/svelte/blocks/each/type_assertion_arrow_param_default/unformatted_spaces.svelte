<script lang="ts">
	let  y  :  unknown  ;
</script>

<!-- an `as` in an arrow's parameter default is an assertion, not the each separator -->
{#each   (  x  =  y  as  A  )  =>  {  }   as   item  }
	<div>{  item  }</div>
{/each}

<!-- in an async arrow's list beside another parameter, with an index and key -->
{#each   async  (  x  =  y  as  A  ,  w  )  =>  {  }   as   item  ,  i   (  i  )  }
	<div>{  item  }</div>
{/each}

<!-- in a function expression's list -->
{#each   function  (  x  =  y  as  A  )  {  }   as   item  }
	<div>{  item  }</div>
{/each}
