<script lang="ts">
	// trailing line comment in stripped union paren
	type A =
		| a // line comment
		| b;

	// trailing line comment in stripped intersection paren
	type B = a & // line comment
		b;
</script>
