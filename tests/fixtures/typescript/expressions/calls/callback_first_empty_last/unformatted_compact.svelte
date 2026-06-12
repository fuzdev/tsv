<script lang="ts">
	// Callback first, empty object last - hugs callback
	fn((x)=>{return x;},{});

	// Callback first, empty array last - hugs callback
	fn((x)=>{return x;},[]);

	// Chained method with callback + empty object
	items.reduce((acc,item)=>{return acc+item;},{});

	// Chained method with callback + empty array
	items.reduce((acc,item)=>{return acc+item;},[]);

	// Type argument on method call with callback + empty object
	items.reduce<T>((acc,item)=>{return acc+item;},{});

	// Multiline callback body with nested blocks
	fn((x)=>{for(const y of x){fn1(y);}return x;},{});

	// Type assertion on empty object - still hugs
	fn((x)=>{return x;},{} as T);

	// Satisfies on empty array - still hugs
	fn((x)=>{return x;},[] satisfies T);

	// New expression with callback + empty object
	new Foo((x)=>{return x;},{});

	// New expression with callback + empty array
	new Bar((x)=>{return x;},[]);
</script>
