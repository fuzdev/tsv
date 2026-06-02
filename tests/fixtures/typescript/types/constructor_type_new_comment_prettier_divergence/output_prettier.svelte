<script lang="ts">
	type A = new () /* foo */ => string;
	type B = new (/* foo */ a: string) => string;
	type C = new /* foo */ <T>() => string;
</script>
