<script lang="ts">
	// A parameter list commits to a function type: the `=>` and return type are
	// required (annotated, empty, bare multiple, optional, and rest forms)
	type Empty = (  )=>A;
	type Annotated = (a:A)=>B;
	type BarePair = (a,b)=>B;
	type Optional = (a?)=>B;
	type Rest = (...a:A[])=>B;
</script>
