<script lang="ts">
	// Unlabeled tuples

	// 100 chars exactly - stays inline (at boundary)
	type A = [  string ,  number ,  boolean ,  object ,  null ,  undefined ,  symbol ,  bigint ,  neverrrrrrrrrrrrrrrr  ];

	// 101 chars - wraps to one element per line
	type B = [
		string  ,
		number  ,
		boolean  ,
		object  ,
		null  ,
		undefined  ,
		symbol  ,
		bigint  ,
		neverrrrrrrrrrrrrrrrr  ,
	];

	// Labeled tuples

	// 100 chars exactly - stays inline (at boundary)
	type C = [  first :  string ,  second :  number ,  third :  boolean ,  fourth :  object ,  fifth :  neverrrrrrrrrrrr  ];

	// 101 chars - wraps to one element per line
	type D = [
		first :  string  ,
		second :  number  ,
		third :  boolean  ,
		fourth :  object  ,
		fifth :  neverrrrrrrrrrrrr  ,
	];
</script>
