<script lang="ts">
	type A = abstract /* foo */ new () => string;
	type B = abstract /* foo */ new (a: string) => string;
	type C = abstract /* foo */ new <T>() => string;
	type D = abstract /* a */ new /* b */ () => string;
</script>
