<script lang="ts">
	type A = { [K in keyof U]:V };
	type B = { [K in keyof U]?:V };
	type C = { readonly [K in keyof U]:V };
	type D = { -readonly [K in keyof U]:V };
	type E = { [K in keyof U]-?:V };
	type F = { [K in keyof U as NewK]:V };
</script>
