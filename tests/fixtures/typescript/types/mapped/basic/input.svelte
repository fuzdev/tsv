<script lang="ts">
	type A = { [K in keyof U]: V };
	type B = { [K in keyof U]?: V };
	type C = { readonly [K in keyof U]: V };
	type D = { -readonly [K in keyof U]: V };
	type E = { +readonly [K in keyof U]: V };
	type F = { [K in keyof U]-?: V };
	type G = { [K in keyof U]+?: V };
	type H = { [K in keyof U as NewK]: V };
</script>
