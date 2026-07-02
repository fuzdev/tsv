<script lang="ts">
	// Mapped types without a value type annotation
	type A = { [K in B]; };
	type C = { [K in B]?; };
	type D = { [K in B]+?; };
	type E = { [K in B]-?; };
	type F = { readonly [K in B]; };
	type G = { [K in B as N]; };
</script>
