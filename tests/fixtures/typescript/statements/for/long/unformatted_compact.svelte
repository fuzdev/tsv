<script lang="ts">
	// Standard for loop - 100 chars exactly (at boundary) - stays inline
	for(let i=0;i<veryLongArrayNameThatExceedsPrintWidthToForceWrappingABCD.length;i=i+1){a;}

	// Standard for loop - 101 chars (exceeds print_width) - wraps
	for(let i=0;i<veryLongArrayNameThatExceedsPrintWidthToForceWrappingABCDE.length;i=i+1){a;}

	// for-of with simple identifier - doesn't wrap (no natural break point)
	for(const item of veryLongArrayNameThatExceedsPrintWidthToForceWrappingABCDEFGHIJKLMNOPQRSTUVWX){a;}

	// for-of with array literal - 100 chars exactly - stays inline
	for(const item of [item1,item2,item3,item4,item5,item6,item7,item8,item9,item10ABCDE]){a;}

	// for-of with array literal - 101 chars - array wraps
	for(const item of [item1,item2,item3,item4,item5,item6,item7,item8,item9,item10ABCDEF]){a;}

	// for-in with simple identifier - doesn't wrap (no natural break point)
	for(const key in veryLongObjectNameThatExceedsPrintWidthToForceWrappingABCDEFGHIJKLMNOPQRSTUVWXY){a;}

	// for-in with object literal - 100 chars exactly - stays inline
	for(const key in {aaa:1,bbb:2,ccc:3,ddd:4,eee:5,fff:6,ggg:7,hhh:8,iiiiiiii:9}){a;}

	// for-in with object literal - 101 chars - object wraps
	for(const key in {aaa:1,bbb:2,ccc:3,ddd:4,eee:5,fff:6,ggg:7,hhh:8,iiiiiiiii:9}){a;}
</script>
