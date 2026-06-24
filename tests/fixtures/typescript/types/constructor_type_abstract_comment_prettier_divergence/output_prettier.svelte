<script lang="ts">
	type A = abstract new () /* foo */ => string;
	type B = abstract new (/* foo */ a: string) => string;
	type C = abstract new /* foo */ <T>() => string;
	type D = abstract new () /* a */ /* b */ => string;
</script>
