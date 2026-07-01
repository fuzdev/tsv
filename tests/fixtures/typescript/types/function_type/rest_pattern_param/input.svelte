<script lang="ts">
	// Rest destructuring-pattern parameters in function and constructor types.
	type F1 = (...[a]) => void;
	type F2 = (...[a]: T[]) => void;
	type F3 = (...{ a }: T) => void;
	type F4 = (b: T, ...[a]) => void;
	type C1 = new (...[a]) => void;
</script>
