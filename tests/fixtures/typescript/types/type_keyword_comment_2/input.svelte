<script lang="ts">
	// Mapped type in keyword
	type A = {[K in /* c */ keyof T]: string};

	// Mapped type as keyword
	type B = {[K in keyof T as /* c */ string]: string};

	// Named tuple member label-to-type
	type C = [label: /* c */ string];

	// Rest type in tuple
	type D = [.../* c */ string[]];

	// Import type qualifier
	type E = import('a')./* c */ Foo;
</script>
