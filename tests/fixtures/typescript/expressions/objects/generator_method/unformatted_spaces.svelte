<script lang="ts">
const  obj  =  {
	*  empty  (  )  {  }  ,
	*  withBody  (  )  {
		yield  1  ;
		yield  2  ;
	}  ,
	*  withParams  (  n  )  {
		for  (  let  i  =  0  ;  i  <  n  ;  i  ++  )  {
			yield  i  ;
		}
	}  ,
	*  [  computed  ]  (  )  {
		yield  1  ;
	}  ,
	*  typed  (  )  :  Generator  <  number  >  {
		yield  1  ;
	}  ,
}  ;
</script>
