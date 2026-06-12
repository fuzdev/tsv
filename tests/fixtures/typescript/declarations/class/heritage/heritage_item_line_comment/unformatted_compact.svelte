<script lang="ts">
	// Line comment after implements item
	class A implements I,// c
	J {}

	// Line comment after implements item with extends
	class B extends C implements I,// c
	J {}

	// Multiple implements items with line comment in middle
	class D implements I,// c
	J, K {}
</script>
