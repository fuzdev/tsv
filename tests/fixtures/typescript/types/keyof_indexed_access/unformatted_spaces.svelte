<script lang="ts">
	type T1 = keyof  A[
		 B ];
	type T2 = keyof  A[ 
		keyof  B 
	];
	type T3 = readonly 
	 A[ B ][];
	const x = y as keyof  A 	[ B ];
</script>
