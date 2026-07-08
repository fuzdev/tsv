<script lang="ts">
	type A = (/* a */ x: T) => void;
	type B = (x /* b */ : T) => void;
	type C = (x: T /* d */) => void;
</script>
