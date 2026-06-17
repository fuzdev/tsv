<script lang="ts">
	type Line = {
		[K in T]  :  V  ;  // c
	};
	type Block = {
		[K in T]  :  V  /* c */  ;
	};
	type Modifiers = {
		readonly  [K in T]  ?:  V  ;  // c
	};
</script>
